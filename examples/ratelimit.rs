use rust_ratelimit::RateLimit;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let start = Instant::now();
    let mut rl = RateLimit::new(Duration::from_millis(1000));
    for _ in 0..10 {
        let start = start.clone();
        rl.spawn(async move {
            println!("{:?}", Instant::now().duration_since(start).as_millis());
        })
        .await;
    }
    rl.wait().await;
}

/*
Output:
0
1004
2008
3010
4013
5014
6016
7019
8021
9025
*/
