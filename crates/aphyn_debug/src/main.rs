use aphyn_app::App;

fn main() {
    let mut app = App::new();

    aphyn_debug::add_module_to(app.ecs_mut());

    for _ in 0..100 {
        app.ecs_mut().data_mut().entities_mut().create_entity();
    }

    app.run();
}
