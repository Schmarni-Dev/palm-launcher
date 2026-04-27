use std::fmt::Debug;

use stardust_xr_asteroids::{CustomElement, FnWrapper, ValidState};
use stardust_xr_fusion::{
    node::NodeError,
    spatial::{Spatial, SpatialRef, Transform},
};

#[allow(clippy::type_complexity)]
#[derive(Debug)]
pub struct SpatialRefExposer<State: ValidState + Debug>(
    FnWrapper<dyn Fn(&mut State, SpatialRef) + Send + Sync>,
);

impl<State: ValidState + Debug> SpatialRefExposer<State> {
    pub fn new(callback: impl Fn(&mut State, SpatialRef) + Send + Sync + 'static) -> Self {
        Self(FnWrapper(Box::new(callback)))
    }
}
impl<State: ValidState + Debug> CustomElement<State> for SpatialRefExposer<State> {
    type Inner = Spatial;

    type Resource = ();

    type Error = NodeError;

    fn create_inner(
        &self,
        _asteroids_context: &stardust_xr_asteroids::Context,
        info: stardust_xr_asteroids::CreateInnerInfo,
        _resource: &mut Self::Resource,
    ) -> Result<Self::Inner, Self::Error> {
        Spatial::create(info.parent_space, Transform::identity())
    }

    fn diff(&self, _old_self: &Self, _inner: &mut Self::Inner, _resource: &mut Self::Resource) {}

    fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
        inner.clone().as_spatial_ref()
    }
    fn frame(
        &self,
        _context: &stardust_xr_asteroids::Context,
        _info: &stardust_xr_fusion::root::FrameInfo,
        state: &mut State,
        inner: &mut Self::Inner,
    ) {
        self.0.0(state, inner.clone().as_spatial_ref());
    }
}
