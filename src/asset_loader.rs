use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, AssetPath, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy::prelude::*;
use bevy::utils::ConditionalSendFuture;
use flume::{Receiver, Sender};

pub struct LuaAssetLoader {
    pub lua_script_rx: Receiver<LuaScript>,
    pub lua_script_bytes_tx: Sender<(Vec<u8>, AssetPath<'static>)>,
}

impl AssetLoader for LuaAssetLoader {
    type Asset = LuaScript;
    type Settings = ();
    type Error = anyhow::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = vec![];
            reader.read_to_end(&mut bytes).await?;
            self.lua_script_bytes_tx
                .send((bytes, load_context.asset_path().clone()))
                .unwrap();
            let lua_script = self.lua_script_rx.recv_async().await?;
            Ok(lua_script)
        })
    }
}

#[derive(TypePath)]
pub struct LuaScript {/*pub exec: SendWrapper<StashedFunction>,*/}

impl VisitAssetDependencies for LuaScript {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

impl Asset for LuaScript {}

#[derive(Resource)]
pub struct LuaAssetCommunicator {
    pub lua_script_tx: Sender<LuaScript>,
    pub lua_script_bytes_rx: Receiver<(Vec<u8>, AssetPath<'static>)>,
}

impl FromWorld for LuaAssetLoader {
    fn from_world(world: &mut World) -> Self {
        let (lua_script_tx, lua_script_rx) = flume::unbounded();
        let (lua_script_bytes_tx, lua_script_bytes_rx) = flume::unbounded();

        world.insert_resource(LuaAssetCommunicator {
            lua_script_tx,
            lua_script_bytes_rx,
        });

        LuaAssetLoader {
            lua_script_rx,
            lua_script_bytes_tx,
        }
    }
}
