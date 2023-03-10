use crate::controller::{stats, task, user};
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn routes() -> Vec<Router> {
    let mut routes = Vec::new();

    // User controller.
    let controller = Router::new()
        .route("/user/new", post(user::new))
        .route("/user/:id/update", post(user::update))
        .route("/user/list", get(user::list))
        .route("/user/:id/view", get(user::view));
    routes.push(controller);

    // Task controller.
    let controller = Router::new().route("/task/execute", post(task::execute));
    routes.push(controller);

    // Stats controller.
    let controller = Router::new().route("/stats", get(stats::index));
    routes.push(controller);

    routes
}
