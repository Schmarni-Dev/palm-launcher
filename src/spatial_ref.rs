use std::fmt::Debug;

use stardust_xr_asteroids::{CustomElement, FnWrapper, ValidState};
use stardust_xr_fusion::{
    node::{NodeError, NodeType},
    objects::SpatialRefProxyExt,
    spatial::{Spatial, SpatialAspect, Transform},
};
use stardust_xr_gluon::{AbortOnDrop, interfaces::SpatialRefProxy};
use stardust_xr_molecules::tracked::TrackedProxy;
use tokio::sync::mpsc;
use tokio_stream::StreamExt as _;

#[allow(clippy::type_complexity)]
#[derive(Debug)]
pub struct ExternalSpatialRef<State: ValidState + Debug> {
    well_known_name: String,
    spatial_path: String,
    tracked_path: Option<String>,
    tracked_changed: FnWrapper<dyn Fn(&mut State, bool) + Send + Sync + 'static>,
}
impl<State: ValidState + Debug> ExternalSpatialRef<State> {
    pub fn new(well_known_name: &str, spatial_path: &str, tracked_path: Option<&str>) -> Self {
        Self {
            well_known_name: well_known_name.to_string(),
            spatial_path: spatial_path.to_string(),
            tracked_path: tracked_path.map(|v| v.to_string()),
            tracked_changed: FnWrapper(Box::new(|_, _| {})),
        }
    }
    pub fn tracked_changed(
        mut self,
        func: impl Fn(&mut State, bool) + Send + Sync + 'static,
    ) -> Self {
        self.tracked_changed = FnWrapper(Box::new(func));
        self
    }
}
pub struct ExternalSpatialRefInner {
    spatial: Spatial,
    tracked_changed_recv: mpsc::UnboundedReceiver<bool>,
    _task: AbortOnDrop,
}
impl<State: ValidState + Debug> CustomElement<State> for ExternalSpatialRef<State> {
    type Inner = ExternalSpatialRefInner;

    type Resource = ();

    type Error = NodeError;

    fn create_inner(
        &self,
        asteroids_context: &stardust_xr_asteroids::Context,
        info: stardust_xr_asteroids::CreateInnerInfo,
        _resource: &mut Self::Resource,
    ) -> Result<Self::Inner, Self::Error> {
        let spatial = Spatial::create(info.parent_space, Transform::identity())?;
        let (tx, rx) = mpsc::unbounded_channel();
        let task = tokio::spawn({
            let spatial = spatial.clone();
            let conn = asteroids_context.dbus_connection.clone();
            let name = self.well_known_name.clone();
            let spatial_path = self.spatial_path.clone();
            let tracked_path = self.tracked_path.clone();
            async move {
                let Ok(spatial_ref) = SpatialRefProxy::new(&conn, name.as_str(), spatial_path)
                    .await
                    .inspect_err(|err| {
                        println!("ERROR: failed to get external spatial ref: {err}")
                    })
                else {
                    return;
                };
                let spatial_ref = spatial_ref.import(spatial.client()).await.unwrap();
                spatial.set_spatial_parent(&spatial_ref).unwrap();
                if let Some(path) = tracked_path
                    && let Ok(proxy) = TrackedProxy::new(&conn, name, path).await
                {
                    tx.send(proxy.is_tracked().await.unwrap_or(true)).unwrap();
                    let mut stream = proxy.receive_is_tracked_changed().await;
                    while let Some(tracked) = stream.next().await {
                        tx.send(tracked.get().await.unwrap_or(true)).unwrap();
                    }
                }
            }
        })
        .into();
        Ok(ExternalSpatialRefInner {
            spatial,
            tracked_changed_recv: rx,
            _task: task,
        })
    }

    fn diff(&self, _old_self: &Self, _inner: &mut Self::Inner, _resource: &mut Self::Resource) {}
    fn frame(
        &self,
        _context: &stardust_xr_asteroids::Context,
        _info: &stardust_xr_fusion::root::FrameInfo,
        state: &mut State,
        inner: &mut Self::Inner,
    ) {
        while let Ok(tracked) = inner.tracked_changed_recv.try_recv() {
            self.tracked_changed.0(state, tracked);
        }
    }

    fn spatial_aspect(&self, inner: &Self::Inner) -> stardust_xr_fusion::spatial::SpatialRef {
        inner.spatial.clone().as_spatial_ref()
    }
}
