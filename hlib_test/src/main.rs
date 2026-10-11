use fruits_engine::{index_version_collection::VersionIndex, *};

mod futures;
mod serialization_macro_check;

fn main() {
    check_all_assets_deserialization();
}

fn check_all_assets_deserialization() {
    fn load_all_assets_system(mut world: WorldDataMut) {
        load_all_assets(world.resources_mut(), "D:/Projects/Hobby/how_many_slices/assets");
    }

    fn init_app(mut world: WorldBuilderMut) {
        world.behavior_mut().get_mut(Schedule::Start).insert_system(load_all_assets_system);
    }

    launch_app_statically(init_app);
}

//

fn check_prefab_serialization() {
    let mut serializers_global = TransSerializerRegistry::new();

    serializers_global.register(StandardSerializer::<String>::default());
    serializers_global.register(StandardSerializer::<u32>::default());
    serializers_global.register(StandardSerializer::<u8>::default());
    serializers_global.register(StandardSerializer::<bool>::default());
    serializers_global.register(StandardSerializer::<i32>::default());
    serializers_global.register(StandardSerializer::<IntComponent>::default());
    serializers_global.register(StandardSerializer::<Vec<ExampleUser>>::default());
    serializers_global.register(StandardSerializer::<ExampleStruct>::default());
    serializers_global.register(StandardSerializer::<ExampleUser>::default());
    serializers_global.register(StandardSerializer::<UserInfo>::default());
    serializers_global.register(StandardSerializer::<SomeComponent>::default());

    //

    let component = ExampleStruct {
        is_verified: true,
        user: ExampleUser::Default {
            name: String::from("Sebastian"),
            age: 44,
        },
        friends: vec![
            ExampleUser::Default {
                name: String::from("Daniel"),
                age: 25,
            },
            ExampleUser::Token(String::from("token_123_abc")),
        ],
    };

    // let component = SomeComponent {
    //     entity: Entity::from_version_index(VersionIndex { index: 1, version: 5 }),
    //     user_info: UserInfo {
    //         name: String::from("winner1337"),
    //         age: 10,
    //     },
    // };

    //

    let entities_deserialized = [
        (25, EntityId::from_version_index(VersionIndex { index: 1, version: 5 })),
    ].into_iter().collect();
    let entities_serialized = [
        (EntityId::from_version_index(VersionIndex { index: 1, version: 5 }), 25),
    ].into_iter().collect();
   
    let mut serializers_local = TransSerializerRegistry::new();

    serializers_local.register(EntityTransSerializer::new(
        &entities_deserialized,
        &entities_serialized,
    ));

    //

    let mut world = {
        let mut world = WorldBuilder::new();
        let mut world_data = world.data_mut();
        let mut ent = world_data.entities_mut();

        let entity = ent.create_entity();

        ent.add_component(entity, component).ok().unwrap();
        ent.add_component(entity, IntComponent(975)).ok().unwrap();

        world.build()
    };

    //

    let entity = world.data().entities().query::<EntityId>().iter().next().unwrap();

    let mut err_handler = |err| println!("{err}");
    let serializer_ctx_state = serializers_global.to_ctx_state();
    let mut serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local)
        .into_ctx(&mut err_handler);

    let prefab = serialize_prefab_single_entity(
        entity,
        serializer_ctx,
        world.data().entities(),
    );

    world.data_mut().entities_mut().destroy_entity(entity);
    let entity = world.data_mut().entities_mut().create_entity();
   
    // deserialize_prefab_single_entity(
    //     &prefab,
    //     entity,
    //     global_serializer.to_ctx(Some(&local_serializer), &mut err_handler),
    //     world.data_mut().entities_mut(),
    // );

    // world.data().entities().get_all_components(entity, |c| {
    //     println!("component {}", c.type_info().short().name());
    // });
   
    // let prefab = serialize_prefab_single_entity(
    //     entity,
    //     global_serializer.to_ctx(Some(&local_serializer), &mut err_handler),
    //     world.data().entities(),
    // );

    // dbg!(&prefab);

    // //

    // let serialized_component = global_serializer.serialize(&component, Some(&local_serializer)).unwrap();

    // dbg!(&serialized_component);
    // println!("{}", serde_json::to_string_pretty(&serialized_component.to_json()).unwrap());

    // let deserialized_component = global_serializer.deserialize::<ExampleStruct>(&serialized_component, Some(&local_serializer)).unwrap();

    // dbg!(&deserialized_component);
    // // dbg!(deserialized_component == Some(component));
}

//

#[derive(Serializable, Default, Debug, Component)]
pub struct SomeComponent {
    pub entity: EntityId,
    pub user_info: UserInfo,
}

#[derive(Serializable, Default, Debug, Component)]
pub struct IntComponent(i32);

#[derive(Serializable, Default, Debug)]
pub struct UserInfo {
    pub name: String,
    pub age: u32,
}

//

#[derive(Serializable, Default, Debug, Component)]
pub struct ExampleStruct {
    is_verified: bool,
    user: ExampleUser,
    friends: Vec<ExampleUser>,
}

#[derive(Serializable, Debug)]
pub enum ExampleUser {
    Default { name: String, age: u8 },
    Token(String),
}

impl Default for ExampleUser {
    fn default() -> Self {
        Self::Default {
            name: Default::default(),
            age: Default::default()
        }
    }
}