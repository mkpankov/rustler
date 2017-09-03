use gender::Gender;
use Options;
use QueryId;
use Storage;
use NewOrUpdateResponse;

use rocket::State;
use rocket::http::Status;
use rocket::response::Failure;
use rocket_contrib::Json;

use std::collections::hash_map::Entry;
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Location {
    pub id: u32,
    pub place: String,
    pub country: String, // [char; 50]
    pub city: String,    // [char; 50]
    pub distance: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct LocationUpdate {
    #[serde(default)] place: String,
    #[serde(default)] country: String, // [char; 50]
    #[serde(default)] city: String,    // [char; 50]
    #[serde(default)] distance: u32,
}

#[derive(FromForm, Debug)]
struct LocationAvgParams {
    #[form(field = "fromDate")] from_date: Option<i32>,
    #[form(field = "toDate")] to_date: Option<i32>,
    #[form(field = "fromAge")] from_age: Option<i32>,
    #[form(field = "toAge")] to_age: Option<i32>,
    gender: Option<Gender>,
}

#[derive(Serialize)]
struct LocationAvg {
    avg: f64,
}

#[get("/locations/<id>")]
fn locations(id: u32, storage: State<Storage>) -> Option<Json<Location>> {
    let locations = &*storage.locations.read().unwrap();
    locations.get(&id).map(|entity| Json(entity.clone()))
}

#[get("/locations/<id>/avg")]
fn locations_avg_no_params(
    id: u32,
    storage: State<Storage>,
    options: State<Options>,
) -> Result<Json<LocationAvg>, Failure> {
    locations_avg(id, None, storage, options)
}

#[get("/locations/<id>/avg?<params>")]
fn locations_avg(
    id: u32,
    params: Option<LocationAvgParams>,
    storage: State<Storage>,
    options: State<Options>,
) -> Result<Json<LocationAvg>, Failure> {
    let all_locations = &storage.locations.read().unwrap();
    {
        if let None = all_locations.get(&id) {
            return Err(Failure(Status::NotFound));
        }
    }

    if let Some(ref params) = params {
        if let Some(ref gender) = params.gender {
            match *gender {
                Gender::Unknown => return Err(Failure(Status::BadRequest)),
                _ => {}
            };
        }

        if params.from_age.is_none() && params.from_date.is_none() && params.gender.is_none() &&
            params.to_age.is_none() && params.to_date.is_none()
        {
            return Err(Failure(Status::BadRequest));
        }
    }

    let all_visits = &storage.visits.read().unwrap();
    let location_visits_ids = &storage.location_visits.read().unwrap();
    let maybe_this_location_visits_ids = location_visits_ids.get(&id);
    let location_visits = if let Some(this_location_visits_ids) = maybe_this_location_visits_ids {
        this_location_visits_ids
            .iter()
            .map(|i| all_visits[&i].clone())
    } else {
        return Ok(Json(LocationAvg { avg: 0. }));
    };

    let result_visits: Vec<_> = if let Some(params) = params {
        let from_date_visits =
            location_visits.filter(|v| if let Some(from_date) = params.from_date {
                if from_date < v.visited_at {
                    true
                } else {
                    false
                }
            } else {
                true
            });

        let to_date_visits = from_date_visits.filter(|v| if let Some(to_date) = params.to_date {
            if to_date > v.visited_at {
                true
            } else {
                false
            }
        } else {
            true
        });

        let from_age_visits = to_date_visits.filter(|v| if let Some(from_age) = params.from_age {
            let users = &storage.users.read().unwrap();
            let user = users.get(&v.user).unwrap();

            let birth_date_timestamp = user.birth_date;
            let age = ::users::calculate_age_from_timestamp(birth_date_timestamp, options.now);

            if from_age <= age {
                true
            } else {
                false
            }
        } else {
            true
        });

        let to_age_visits = from_age_visits.filter(|v| if let Some(to_age) = params.to_age {
            let users = &storage.users.read().unwrap();
            let user = users.get(&v.user).unwrap();

            let birth_date_timestamp = user.birth_date;
            let age = ::users::calculate_age_from_timestamp(birth_date_timestamp, options.now);

            if to_age > age {
                true
            } else {
                false
            }
        } else {
            true
        });

        let final_visits = to_age_visits.filter(|v| if let Some(ref gender) = params.gender {
            let users = &storage.users.read().unwrap();
            let user = users.get(&v.user).unwrap();
            let reference_gender = &user.gender;

            if gender == reference_gender {
                true
            } else {
                false
            }
        } else {
            true
        });

        final_visits.collect()
    } else {
        location_visits.collect()
    };

    let marks = result_visits.iter().map(|v| v.mark);
    let mut sum: usize = 0;
    for v in marks {
        sum += v as usize;
    }
    let avg_mark: f64 = if sum > 0 {
        sum as f64 / result_visits.len() as f64
    } else {
        0.0
    };
    let avg_mark_rounded = format!("{:.5}", avg_mark);
    let avg_mark_rounded: f64 = avg_mark_rounded.parse().unwrap();

    Ok(Json(LocationAvg {
        avg: avg_mark_rounded,
    }))
}

#[post("/locations/<id>?<query_id>", data = "<location>")]
fn locations_update(
    id: u32,
    location: Json<LocationUpdate>,
    query_id: QueryId,
    storage: State<Storage>,
) -> Option<Json<NewOrUpdateResponse>> {
    let _query_id = query_id;
    let location_update = location.0;

    let locations = &mut storage.locations.write().unwrap();
    let location_entry = locations.entry(id);
    match location_entry {
        Entry::Occupied(mut e) => {
            if location_update.city != "" {
                e.get_mut().city = location_update.city;
            }
            if location_update.country != "" {
                e.get_mut().country = location_update.country;
            }
            if location_update.distance != 0 {
                e.get_mut().distance = location_update.distance;
            }
            if location_update.place != "" {
                e.get_mut().place = location_update.place;
            }
        }
        Entry::Vacant(_) => return None,
    }
    Some(Json(NewOrUpdateResponse))
}

#[post("/locations/new", data = "<location>")]
fn locations_new(
    location: Json<Location>,
    storage: State<Storage>,
) -> Option<Json<NewOrUpdateResponse>> {
    let location = location.0;
    let id = location.id;

    let locations = &mut storage.locations.write().unwrap();
    let location_entry = locations.entry(id);
    match location_entry {
        Entry::Occupied(_) => return None,
        Entry::Vacant(e) => {
            e.insert(Location {
                id: id,
                city: location.city,
                country: location.country,
                distance: location.distance,
                place: location.place,
            });
        }
    }
    Some(Json(NewOrUpdateResponse))
}
