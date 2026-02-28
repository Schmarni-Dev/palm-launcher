use stardust_xr_fusion::{ClientHandle, fields::FieldRef, spatial::SpatialRef};
use stardust_xr_gluon::query::ObjectQuery;
use stardust_xr_molecules::{Derezzable, DerezzableHandlerProxy};

pub struct Derezzer {}

struct DerezzerInner {
    query: ObjectQuery<
        (
            DerezzableHandlerProxy<'static>,
            Option<FieldRef>,
            SpatialRef,
        ),
        ClientHandle,
    >,
}
