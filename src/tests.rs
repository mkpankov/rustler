use rocket::local::Client;
use rocket::http::Status;
use super::*;

fn setup() -> rocket::Rocket {
    let data = input_data(&PathBuf::from("data")).unwrap();
    let data_locked = RwLock::new(data);
    rocket::ignite().manage(data_locked).mount(
        "/",
        routes![
            users,
            locations,
            visits,
            users_visits_no_params,
            users_visits,
            locations_avg_no_params,
            locations_avg,
            users_update,
            locations_update,
            visits_update,
            users_new,
            locations_new,
            visits_new,
        ],
    )
}

#[test]
fn users_id() {
    let rocket = setup();
    let client = Client::new(rocket).expect("valid rocket instance");
    let mut response = client.get("/users/1").dispatch();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.body_string(), Some("{\"id\":1,\"email\":\"iwgeodwa@list.me\",\"first_name\":\"Инна\",\"last_name\":\"Терыкатева\",\"gender\":\"f\",\"birth_date\":-712108800}".into()));
}

#[test]
fn users_string() {
    let rocket = setup();
    let client = Client::new(rocket).expect("valid rocket instance");
    let response = client.get("/users/string").dispatch();
    assert_eq!(response.status(), Status::NotFound);
}

#[test]
fn users_string_somethingbad() {
    let rocket = setup();
    let client = Client::new(rocket).expect("valid rocket instance");
    let response = client.get("/users/string/somethingbad").dispatch();
    assert_eq!(response.status(), Status::NotFound);
}

#[test]
fn users() {
    let rocket = setup();
    let client = Client::new(rocket).expect("valid rocket instance");
    let response = client.get("/users/").dispatch();
    assert_eq!(response.status(), Status::NotFound);
}

#[test]
fn user() {
    let rocket = setup();
    let client = Client::new(rocket).expect("valid rocket instance");
    let response = client.get("/user/").dispatch();
    assert_eq!(response.status(), Status::NotFound);
}
