#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;

use std::error::Error;

#[get("/users/<id>")]
fn users(id: u32) -> String {
    format!("user {}", id)
}

#[get("/users/<id>/visit")]
fn users_visit(id: u32) -> String {
    format!("user {} visit", id)
}

#[post("/users/<id>")]
fn users_update(id: u32) -> String {
    format!("user {} update", id)
}

#[post("/users/new")]
fn users_new() -> String {
    format!("user new")
}

#[get("/locations/<id>")]
fn locations(id: u32) -> String {
    format!("locations {}", id)
}

#[get("/locations/<id>/avg")]
fn locations_avg(id: u32) -> String {
    format!("locations {} avg", id)
}

#[post("/locations/<id>")]
fn locations_update(id: u32) -> String {
    format!("location {} update", id)
}

#[post("/locations/new")]
fn locations_new() -> String {
    format!("locations new")
}

#[get("/visits/<id>")]
fn visits(id: u32) -> String {
    format!("visit {}", id)
}

#[post("/visits/<id>")]
fn visits_update(id: u32) -> String {
    format!("visit {} update", id)
}

#[get("/visits/new")]
fn visits_new() -> String {
    format!("visit new")
}

fn work() -> Result<(), Box<Error>> {
    rocket::ignite()
        .mount(
            "/",
            routes![
                users,
                locations,
                visits,
                users_visit,
                locations_avg,
                users_update,
                locations_update,
                visits_update,
            ],
        )
        .launch();
    Ok(())
}

fn main() {
    work().unwrap();
}
