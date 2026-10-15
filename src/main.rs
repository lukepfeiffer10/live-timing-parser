mod providers;
use std::{collections::HashMap, sync::{Arc, Mutex, RwLock}};

use chrono::{Duration, prelude::*};
use log::{info, error, trace};
use tokio::task::JoinHandle;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use warp::{Filter};

use crate::providers::ris::RISTimingProvider;

#[tokio::main]
async fn main() {
    env_logger::init();

    let shutdown_token = CancellationToken::new();
    let cloned_shutdown_token = shutdown_token.clone();
    
    let hello = warp::path::end()
        .map(|| "Start parsing live data by POSTing to the /start endpoint. \n
            Calling the GET /{id}/stats with the id returned from /start will get the saved timing data.");

    
    let timing_id: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let cloned_timing_id = timing_id.clone();
    let lap_time_data: Arc<RwLock<HashMap<i32, Vec<Duration>>>> = Arc::new(RwLock::new(HashMap::new()));
    let cloned_lap_time_data = lap_time_data.clone();
    let update_data = move |car_number: i32, lap_time: Duration| {
        if let Ok(mut lap_time_data) = cloned_lap_time_data.write() {
            if let Some(lap_times) = lap_time_data.get_mut(&car_number) {
                lap_times.push(lap_time);
            } else {
                let mut lap_times = Vec::with_capacity(100);
                lap_times.push(lap_time);
                lap_time_data.insert(car_number, lap_times);
            }
        }
    };
    let update_data_ref = Arc::new(update_data);
    let update_data_clone = update_data_ref.clone();

    let cloned_data = lap_time_data.clone();
    let start = warp::path("start")
        .map(move || {
            let cloned_token = cloned_shutdown_token.clone();
            let cloned_data = cloned_data.clone();
            let update_data_clone = update_data_clone.clone();
            let mut lock = cloned_timing_id.try_lock();
            if let Ok(ref mut mutex) = lock {
                if let None = &**mutex {
                    let handle = tokio::spawn(async move {
                        info!("started the live timing");
                        let ris_timing = RISTimingProvider::new();
                        let mut current_lap_numbers: HashMap<i32, i32> = HashMap::new();
                        loop {
                            if cloned_token.is_cancelled() {
                                trace!("cancelling the polling task");
                                
                                println!("{:#?}", cloned_data.read().unwrap());
                                break;
                            }
                            let timing_point_result = ris_timing.get_timing_point().await;

                            match timing_point_result {
                                Ok(timing_point) => {
                                    timing_point.cars
                                        .iter()
                                        .for_each(|car_timing_point| {
                                            let car_number = car_timing_point.car_number;
                                            if let Some(current_lap) = current_lap_numbers.get_mut(&car_number) {
                                                if *current_lap != car_timing_point.lap.lap_number {
                                                    let lap_time: Duration = car_timing_point.lap.sector_times
                                                        .iter()
                                                        .map(|sector_time| sector_time.map_or(Duration::zero(), |sector_time| sector_time))
                                                        .sum();

                                                    update_data_clone(car_number, lap_time);

                                                    if car_number == 500 {
                                                        info!("Car 500 lap time: {:#?}", lap_time);
                                                    }
                                                    
                                                    *current_lap = car_timing_point.lap.lap_number;
                                                }
                                            }
                                            else {
                                                current_lap_numbers.insert(car_number, car_timing_point.lap.lap_number);
                                            }
                                        });
                                },
                                Err(e) => error!("Failed to get live timing data: {:#?}", e)
                            }                            

                            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                            trace!("polling live timing data");
                        }
                    });
                    **mutex = Some(handle);
                }
                warp::reply::with_status("started parsing live timing", warp::http::StatusCode::ACCEPTED)
            } else {
                error!("try_lock failed");
                warp::reply::with_status("failed to start parsing live timing", warp::http::StatusCode::INTERNAL_SERVER_ERROR)
            }            
        });

    let routes = warp::get().and(
        hello
    ).or(
        warp::post().and(start)
    );    

    let c_timing_id = timing_id.clone();
    warp::serve(routes)
        .bind(([127, 0, 0, 1], 3000))
        .await
        .graceful(async move {
            match signal::ctrl_c().await {
                Ok(()) => {
                    shutdown_token.cancel();
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let lock = c_timing_id.try_lock();
                    if let Ok(ref mutex) = lock {
                        if let Some(handle) = &**mutex {
                            handle.abort();
                        }
                    } else {
                        error!("try_lock failed");
                    }
                },
                Err(err) => {
                    eprintln!("Unable to listen for shutdown signal: {}", err);
                },
            }
        })
        .run()
        .await;
}

trait TimingProvider {
    async fn get_timing_point(&self) -> Result<TimingPoint, Box<dyn std::error::Error + Send + Sync>>;
}

struct TimingPoint {
    sequence_number: u64,
    cars: Vec<CarTimingPoint>
}

#[derive(Clone, Debug)]
struct CarTimingPoint {
    team: String,
    driver: String,
    position: i32,
    car_number: i32,
    lap: LapTimingPoint
}

#[derive(Clone, Debug)]
struct LapTimingPoint {
    sector_times: Vec<Option<Duration>>,
    lap_start: Option<DateTime<Utc>>,
    lap_end: Option<DateTime<Utc>>,
    best_sector_times: Vec<Option<Duration>>,
    car_state: String,
    lap_number: i32,
    best_lap_time: Option<Duration>
}
