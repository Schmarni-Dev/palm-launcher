use glam::{Vec3, vec2};
use stardust_xr_asteroids::{Context, CustomElement, ValidState};
use stardust_xr_fusion::{
    ClientHandle,
    fields::{FieldRef, FieldRefAspect},
    node::{NodeError, NodeType},
    root::FrameInfo,
    spatial::{Spatial, SpatialAspect, SpatialRef, SpatialRefAspect, Transform},
    values::{Quaternion, Vector3},
};
use stardust_xr_gluon::{ObjectEventStreamExt, WatchHandle};
use stardust_xr_molecules::DerezzableHandlerProxy;

#[derive(Debug)]
pub struct Derezzer {
    pos: Vector3<f32>,
    rot: Quaternion,
    length: f32,
}
impl Derezzer {
    pub fn new(pos: impl Into<Vector3<f32>>, rot: impl Into<Quaternion>, length: f32) -> Self {
        Self {
            pos: pos.into(),
            rot: rot.into(),
            length,
        }
    }
}

impl<State: ValidState> CustomElement<State> for Derezzer {
    type Inner = DerezzerInner;

    type Resource = ();

    type Error = NodeError;

    fn create_inner(
        &self,
        asteroids_context: &Context,
        info: stardust_xr_asteroids::CreateInnerInfo,
        _resource: &mut Self::Resource,
    ) -> Result<Self::Inner, Self::Error> {
        let client = info.parent_space.client();
        let query = asteroids_context
            .object_registry
            .query::<_, ClientHandle>(client.clone())
            .watch();
        let spatial = Spatial::create(
            info.parent_space,
            Transform::from_translation_rotation(self.pos, self.rot),
        )?;
        Ok(DerezzerInner { spatial, query })
    }

    fn diff(&self, old_self: &Self, inner: &mut Self::Inner, _resource: &mut Self::Resource) {
        if self.pos != old_self.pos {
            _ = inner
                .spatial
                .set_local_transform(Transform::from_translation(self.pos));
        }
        if self.rot != old_self.rot {
            _ = inner
                .spatial
                .set_local_transform(Transform::from_rotation(self.rot));
        }
    }
    fn frame(
        &self,
        _context: &Context,
        _info: &FrameInfo,
        _state: &mut State,
        inner: &mut Self::Inner,
    ) {
        let ref_space_spatial = inner.spatial.clone();
        let distance = self.length;
        let derezzables = inner
            .query
            .watch
            .borrow()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        tokio::spawn({
            async move {
                for (derezzable, field, spatial) in derezzables {
                    if let Some(field) = field {
                        if field
                            .ray_march(&ref_space_spatial, Vec3::ZERO, Vec3::Y)
                            .await
                            .is_ok_and(|v| {
                                v.min_distance <= 0.001 && v.deepest_point_distance <= distance
                            })
                        {
                            _ = derezzable.derez().await;
                        }
                    } else {
                        let Ok(transform) = spatial.get_transform(&ref_space_spatial).await else {
                            continue;
                        };
                        if transform.translation.is_some_and(|p| {
                            dbg!(p);
                            p.y <= distance && p.y >= 0.0 && vec2(p.x, p.z).length() < 0.01
                        }) {
                            _ = derezzable.derez().await;
                        }
                    }
                }
            }
        });
    }

    fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
        inner.spatial.clone().as_spatial_ref()
    }
}

pub struct DerezzerInner {
    query: WatchHandle<(
        DerezzableHandlerProxy<'static>,
        Option<FieldRef>,
        SpatialRef,
    )>,
    spatial: Spatial,
}
