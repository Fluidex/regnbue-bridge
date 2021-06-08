use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use heimdallr::tele_out::{storage, Settings};
use std::cell::RefCell;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();
    log::info!("heimdallr tele_out started");

    let mut conf = config_rs::Config::new();
    let config_file = dotenv::var("TELE_OUT_CONFIG").unwrap();
    conf.merge(config_rs::File::with_name(&config_file)).unwrap();
    let settings: Settings = conf.try_into().unwrap();
    log::debug!("{:?}", settings);

    // handle ctrl+c
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("crtlc signal send");
        })
        .expect("Error setting Ctrl-C handler");
    }

    let _dbpool = storage::from_config(&settings).await?;

    // TODO: this seems using panic to notify the main thread. should we use the dingir-exchange style?
    let fetcher_task_handle = run_task_fetcher(stop_signal_sender);
    let eth_sender_task_handle = run_eth_sender(stop_signal_sender);

    tokio::select! {
        _ = async { fetcher_task_handle.await } => {
            panic!("Tele_out task fetcher actor is not supposed to finish its execution")
        },
        _ = async { eth_sender_task_handle.await } => {
            panic!("Ethereum Sender actor is not supposed to finish its execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            log::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
