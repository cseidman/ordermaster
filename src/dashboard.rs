use clap::Parser;
use proto::orderbook_aggregator_client::OrderbookAggregatorClient;
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};

// UI related uses
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{
        enable_raw_mode,
        disable_raw_mode,
        EnterAlternateScreen,
        LeaveAlternateScreen,
    },

};

use std::{
    io::{stdout},
    time::{Duration, Instant},
};

use tui::{
    backend::{Backend, CrosstermBackend},
    Frame, layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    symbols,
    Terminal,
    text::{Span, Spans},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Wrap}
};

mod proto {
    tonic::include_proto!("orderbook");
}

#[derive(Parser)]
struct Cli {
    #[clap(short, long, help = "(Optional) Port number of the gRPC server. Default: 33333")]
    port: Option<usize>,
}

#[derive(Clone)]
struct Datapoint {
    price: Decimal,
    qty: Decimal,
    exchange: String
}

struct App {
    bid: Decimal,
    ask: Decimal,
    spread: Decimal,
    bids: Vec<Datapoint>,
    asks: Vec<Datapoint>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res = run_app(&mut terminal);

    if let Err(err) = res.await {
        println!("{:?}", err)
    }
    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> Result<(), Box<dyn std::error::Error>> {

    let args = Cli::parse();
    let port: usize = args.port.unwrap_or(33333);
    let addr = format!("http://[::1]:{}", port);

    env_logger::init();

    let mut client = OrderbookAggregatorClient::connect(addr).await.unwrap();

    let request = tonic::Request::new(proto::Empty {});

    let mut response = client.book_summary(request).await?.into_inner();

    // listening to stream
    let mut bid_data:Vec<Datapoint> = Vec::new() ;
    let mut ask_data:Vec<Datapoint> = Vec::new() ;

    while let Some(res) = response.message().await? {

        let last_tick = Instant::now();

        let tick_rate = Duration::from_millis(100);

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
            }
        }

        let proto::Summary{spread, bids, asks} = res;

        // set spread
        let mut spread = Decimal::from_f64(spread).unwrap() ;
        spread.rescale(8);

        // set bids

        bids.iter().for_each(|level|
            {
                bid_data.insert(0, Datapoint {
                    price: Decimal::from_f64(level.price).unwrap(),
                    qty: Decimal::from_f64(level.amount).unwrap(),
                    exchange: level.exchange.clone()
                });
                if bid_data.len() > 200 {
                    bid_data.pop();
                }
            }
        );

        // set asks

        asks.iter().for_each(|level|
            {
                ask_data.insert(0, Datapoint {
                    price: Decimal::from_f64(level.price).unwrap(),
                    qty: Decimal::from_f64(level.amount).unwrap(),
                    exchange: level.exchange.clone()
                });
                if ask_data.len() > 200 {
                    ask_data.pop();
                }
            }
        );

        let app = App {
            bid: bid_data.first().unwrap().price ,
            ask: ask_data.first().unwrap().price ,
            spread,
            bids: bid_data.clone(),
            asks: ask_data.clone(),
        } ;
        terminal.draw(|f| ui(f, &app))?;

    }

    Ok(())

}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)

        .constraints(
            [
                Constraint::Percentage(10),
                Constraint::Percentage(50),
                Constraint::Percentage(40),
            ]
            .as_ref(),
        )
        .split(f.size());

    // Title
    let title_block = Block::default().title("Market").borders(Borders::ALL);
   // let market_data = format!("Bid: {} Ask: {} Spread: {}", app.bid, app.ask, app.spread) ;
    let text = vec![
        Spans::from(vec![
            Span::styled("Bid: ",Style::default().fg(Color::White)),
            Span::styled(format!("{} ",app.bid), Style::default().fg(Color::Cyan)),

            Span::styled(" Ask: ",Style::default().fg(Color::White)),
            Span::styled(format!("{} ",app.ask), Style::default().fg(Color::Magenta)),

            Span::styled(" Spread: ", Style::default().fg(Color::White)),
            Span::styled(format!("{} ",app.spread),Style::default().fg(Color::Yellow)),

        ]),
    ];
    let p = Paragraph::new(text)
        .block(title_block)
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(p, chunks[0]);

    // Chart rendering

    let mut loc:f64 = 0.0;

    let mut bvec : Vec<(f64,f64)> = Vec::new() ;
    for b in app.bids.clone(){
        loc+=1.0 ;
        bvec.push((loc, b.qty.to_f64().unwrap()))
    }

    loc = 0.0 ;
    let mut avec : Vec<(f64,f64)> = Vec::new() ;
    for a in app.asks.clone() {
        loc+=1.0 ;
        avec.push((loc, a.qty.to_f64().unwrap()))

    }

    let chart_bids = bvec.as_slice() ;
    let chart_asks = avec.as_slice() ;

    let datasets = vec![

        Dataset::default()
            .name("Bid")
            .marker(symbols::Marker::Block)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(chart_bids),

        Dataset::default()
            .name("Ask")
            .marker(symbols::Marker::Block)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Magenta))
            .data(chart_asks),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(Span::styled(
                    "Bid/Ask",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title("X Axis")
                .style(Style::default().fg(Color::Gray))
                .bounds([1.0, 200.0]),
        )
        .y_axis(
            Axis::default()
                .title("Volume")
                .style(Style::default().fg(Color::Gray))
                .labels(vec![
                    Span::styled("0", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("25"),
                    Span::styled("50", Style::default().add_modifier(Modifier::BOLD)),
                ])
                .bounds([0.0, 50.0]),
        );

    f.render_widget(chart, chunks[1]);

    // Footer info

    let mut text: Vec<Spans> = Vec::new() ;
    app.bids.iter().enumerate().for_each(|(i,f)|{
        let line = format!("Bid: {:12}  Amount {:12} | {}", f.price, f.qty, f.exchange) ;

        let bid_span = Spans::from(Span::styled(
            line,
            Style::default().fg(Color::Cyan),
        )) ;

        text.push(bid_span) ;

        let asks = app.asks[i].clone() ;

        let line = format!("Ask: {:12}  Amount {:12} | {}", asks.price, asks.qty, asks.exchange) ;

        let ask_span = Spans::from(Span::styled(
            line,
            Style::default().fg(Color::Magenta),
        )) ;

        text.push(ask_span) ;


    }) ;

    let create_block = |title| {
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Gray))
            .title(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
    };

    let paragraph = Paragraph::new(text.clone())
        .style(Style::default().fg(Color::Gray))
        .block(create_block("Order Book Stream"))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        ;
    f.render_widget(paragraph, chunks[2]);
}

