use chrono::{DateTime, Duration, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{CarTimingPoint, LapTimingPoint, TimingPoint, TimingProvider};

static BASE_URL:&str = "https://live.ris-timing.be/api";

pub struct RISTimingProvider {
    client: Client,
}

impl RISTimingProvider {    
    pub fn new() -> RISTimingProvider {
        let client = Client::builder()
            .cookie_store(true)
            .build()
            .unwrap();

        RISTimingProvider {
            client
        }
    }
}

impl TimingProvider for RISTimingProvider {    
    async fn get_timing_point(&self) -> Result<crate::TimingPoint, Box<dyn std::error::Error + Send + Sync>> {
        let timing_suffix = "live-timing?uuid=00000000-0000-0000-0000-000000000005";
        let timing_url = format!("{}/{}", BASE_URL, timing_suffix);

        // println!("{} {}", "Timing url:", timing_url.as_str());

        let response = self.client.get(timing_url.as_str())
            .send()
            .await?;

        if response.status() == StatusCode::FORBIDDEN {
            if authenticate(&self.client).await? {
                let response_message = self.client.get(timing_url)
                    .send()
                    .await?
                    .text()
                    .await?;

                println!("{:?},", response_message);

                let json = serde_json::from_str::<RaceData>(&response_message)?;
                return Ok(json.to_timing_point());
            }
            
            return Err("Couldn't authenticate to the live timing service".into());
        }

        let message = response
            .text()
            .await?;

        println!("{:?},", message);

        let json = serde_json::from_str::<RaceData>(&message)?;
        return Ok(json.to_timing_point());
    }
}

async fn authenticate(client: &Client) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let auth_suffix = "refresh-session";
    let auth_url = format!("{}/{}", BASE_URL, auth_suffix);
    // println!("{} {}", "Auth url:", auth_url.as_str());

    let response = client.post(auth_url)
        .send()
        .await?;

    if response.status() == StatusCode::OK {
        return Ok(true);
    }

    return Ok(false);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RaceData {
    pub seq: u64,
    pub cars: Vec<Car>,
    pub events: Vec<Event>,
    pub context: Context,
    pub provider: String,
    pub generated_at: DateTime<Utc>,
    pub schema_version: String,
}

impl RaceData {
    pub fn to_timing_point(&self) -> TimingPoint {        
        TimingPoint { 
            sequence_number: self.seq, 
            cars: self.cars.iter()
                    .map(|car| car.to_car_timing_point())
                    .collect()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Car {
    pub lap: Lap,
    pub gaps: Gaps,
    pub ints: Ints,
    pub team: Option<String>,
    pub stops: Stops,
    pub driver: Option<String>,
    pub category: Option<String>,
    pub position: i32,
    pub car_number: i32,
    pub manufacturer: Option<String>,
}

impl Car {
    pub fn to_car_timing_point(&self) -> CarTimingPoint {
        CarTimingPoint { 
            team: self.team.clone().map_or(String::from("unknown"), |name| name), 
            driver: self.driver.clone().map_or(String::from("unknown"), |driver| driver), 
            position: self.position,
            car_number: self.car_number, 
            lap: self.lap.to_lap_timing_point()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Lap {
    pub s1_ms: Option<i32>,
    pub s2_ms: Option<i32>,
    pub s3_ms: Option<i32>,
    pub lap_end: Option<DateTime<Utc>>,
    pub car_state: Option<String>,
    pub lap_start: Option<DateTime<Utc>>,
    pub lap_number: Option<i32>,
    pub s1_best_ms: Option<i32>,
    pub s2_best_ms: Option<i32>,
    pub s3_best_ms: Option<i32>,
    pub best_lap_ms: Option<i32>,
    pub lap_end_xml: Option<String>,
    pub lap_time_ms: Option<i32>,
    pub pit_in_time: Option<String>,
    pub pit_out_time: Option<String>,
}

impl Lap {
    pub fn to_lap_timing_point(&self) -> LapTimingPoint {
        LapTimingPoint { 
            sector_times: vec![
                self.s1_ms.map(|time| Duration::milliseconds(time.into())),
                self.s2_ms.map(|time| Duration::milliseconds(time.into())),
                self.s3_ms.map(|time| Duration::milliseconds(time.into()))
            ], 
            lap_start: self.lap_start.clone(), 
            lap_end: self.lap_end.clone(), 
            best_sector_times: vec![
                self.s1_best_ms.map(|time| Duration::milliseconds(time.into())),
                self.s2_best_ms.map(|time| Duration::milliseconds(time.into())),
                self.s3_best_ms.map(|time| Duration::milliseconds(time.into()))
            ], 
            car_state: self.car_state.clone().map_or(String::from("unknown"), |state| state), 
            lap_number: self.lap_number.map_or(0, |lap| lap), 
            best_lap_time: self.best_lap_ms.map(|time| Duration::milliseconds(time.into()))
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Gaps {
    #[serde(rename = "toLeader")]
    pub to_leader: ToLeader,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ToLeader {
    pub ms: Option<i32>,
    pub laps: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Ints {
    #[serde(rename = "toAhead")]
    pub to_ahead: ToAhead,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ToAhead {
    pub ms: Option<i32>,
    pub laps: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Stops {
    pub count: Option<i32>,
    pub total_time_ms: Option<i32>,
    pub last_stop_duration_ms: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Event {
    pub at: Option<String>,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Context {
    pub race: Race,
    pub season: Season,
    pub series: Series,
    pub session: Session,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Race {
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Season {
    pub year: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Series {
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Session {
    pub name: Option<String>,
    pub clock: Clock,
    pub status: Option<String>,
    pub track_state: Option<String>,
    pub session_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Clock {
    pub start_at: Option<String>,
    pub elapsed_ms: i64,
    pub remaining_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapping_data() {
        let json = r#"{
            "seq": 1791623854657,
            "cars": [
                {
                    "lap": {
                        "s1_ms": 211039,
                        "s2_ms": 108795,
                        "s3_ms": 77628,
                        "lap_end": "2026-10-10T09:17:02.211045+00:00",
                        "car_state": "RUN",
                        "lap_start": "2026-10-10T09:10:24.749045+00:00",
                        "lap_number": 44,
                        "s1_best_ms": 34990,
                        "s2_best_ms": 40298,
                        "s3_best_ms": 34266,
                        "best_lap_ms": 110867,
                        "lap_end_xml": null,
                        "lap_time_ms": 397462,
                        "pit_in_time": null,
                        "pit_out_time": null
                    },
                    "gaps": {
                        "toLeader": {
                            "ms": 758,
                            "laps": 0
                        }
                    },
                    "ints": {
                        "toAhead": {
                            "ms": 758,
                            "laps": null
                        }
                    },
                    "team": "Clubsport Racing_415",
                    "stops": {
                        "count": 7,
                        "total_time_ms": 2505000,
                        "last_stop_duration_ms": 139000
                    },
                    "driver": "S.MES",
                    "category": "EV3",
                    "position": 2,
                    "car_number": 415,
                    "manufacturer": "Fun Cup"
                },
                {
                    "lap": {
                        "s1_ms": 202922,
                        "s2_ms": 111332,
                        "s3_ms": 78851,
                        "lap_end": "2026-10-10T09:17:02.211146+00:00",
                        "car_state": "RUN",
                        "lap_start": "2026-10-10T09:10:29.106146+00:00",
                        "lap_number": 51,
                        "s1_best_ms": 35404,
                        "s2_best_ms": 40609,
                        "s3_best_ms": 33600,
                        "best_lap_ms": 112245,
                        "lap_end_xml": null,
                        "lap_time_ms": 393105,
                        "pit_in_time": null,
                        "pit_out_time": null
                    },
                    "gaps": {
                        "toLeader": {
                            "ms": 2136,
                            "laps": 0
                        }
                    },
                    "ints": {
                        "toAhead": {
                            "ms": 1378,
                            "laps": null
                        }
                    },
                    "team": "Academy by M3M 2",
                    "stops": {
                        "count": 6,
                        "total_time_ms": 780000,
                        "last_stop_duration_ms": 129000
                    },
                    "driver": "E.THI",
                    "category": "EV3",
                    "position": 3,
                    "car_number": 560,
                    "manufacturer": "Fun Cup"
                }
            ],
            "events": [
                {
                    "at": "2026-10-10T09:17:34.658076+00:00",
                    "id": "dc2-msg",
                    "kind": "RC_MESSAGE",
                    "message": "FCY REGULATION"
                }
            ],
            "context": {
                "race": {
                    "name": "No race name"
                },
                "season": {
                    "year": 2026
                },
                "series": {
                    "name": "Fun Cup BE"
                },
                "session": {
                    "name": "Paying Practice 1",
                    "clock": {
                        "start_at": null,
                        "elapsed_ms": 8248000,
                        "remaining_ms": 1952000
                    },
                    "status": "RUNNING",
                    "track_state": "GREEN",
                    "session_type": "PRACTICE"
                }
            },
            "provider": "RIS",
            "generated_at": "2026-10-10T09:17:34.657969+00:00",
            "schema_version": "1.0"
        }"#;

        let deserialized_race_data = serde_json::from_str::<RaceData>(&json).unwrap();
        let mapped_data = deserialized_race_data.to_timing_point();

        assert_eq!(1791623854657, mapped_data.sequence_number, "sequence number mismatch");
        assert_eq!(415, mapped_data.cars[0].car_number, "car number mismatch");
        assert_eq!("S.MES", mapped_data.cars[0].driver, "driver name mismatch");
        assert_eq!(44, mapped_data.cars[0].lap.lap_number, "lap number mismatch");
        assert_eq!(110867, mapped_data.cars[0].lap.best_lap_time.unwrap().num_milliseconds(), "best lap time mismatch");
        assert_eq!(211039, mapped_data.cars[0].lap.sector_times[0].unwrap().num_milliseconds(), "first sector time mismatch");
        assert_eq!(108795, mapped_data.cars[0].lap.sector_times[1].unwrap().num_milliseconds(), "second sector time mismatch");
        assert_eq!(77628, mapped_data.cars[0].lap.sector_times[2].unwrap().num_milliseconds(), "third sector time mismatch");
    }
}