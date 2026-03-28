use clap::Parser;
use prettytable::{Table, row};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Initial number of subscribers
    #[arg(short, long, default_value_t = 100)]
    subscribers: u32,

    /// Monthly growth rate in percentage (e.g., 10 for 10%)
    #[arg(short, long, default_value_t = 15.0)]
    growth: f64,

    /// Monthly churn rate in percentage (e.g., 5 for 5%)
    #[arg(short, long, default_value_t = 5.0)]
    churn: f64,

    /// Monthly subscription fee in dollars
    #[arg(short, long, default_value_t = 10.0)]
    fee: f64,

    /// Number of months to simulate
    #[arg(short, long, default_value_t = 12)]
    months: u32,
}

fn main() {
    let args = Args::parse();
    
    let mut current_subs = args.subscribers as f64;
    let growth_rate = args.growth / 100.0;
    let churn_rate = args.churn / 100.0;
    
    let mut table = Table::new();
    table.add_row(row!["Month", "New Subs", "Lost Subs", "Net Subs", "Revenue ($)"]);

    for month in 1..=args.months {
        let newcomers = current_subs * growth_rate;
        let churners = current_subs * churn_rate;
        let net_growth = newcomers - churners;
        current_subs += net_growth;
        
        let monthly_revenue = current_subs * args.fee;
        
        table.add_row(row![
            month,
            format!("{:.1}", newcomers),
            format!("{:.1}", churners),
            format!("{:.0}", current_subs.max(0.0)),
            format!("{:.2}", monthly_revenue.max(0.0))
        ]);
    }
    
    println!("\nSubStream Creator Revenue Simulator");
    println!("----------------------------------");
    println!("Initial Subs: {}", args.subscribers);
    println!("Monthly Growth: {}%", args.growth);
    println!("Monthly Churn: {}%", args.churn);
    println!("Subscription Fee: ${}", args.fee);
    println!("\nProjection:");
    table.printstd();
}
