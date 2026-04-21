pub mod derezzer;
pub mod spatial_ref;
pub mod spatial_ref_exposer;

use std::{
    env,
    f32::consts::{FRAC_PI_2, FRAC_PI_3, PI},
    process::{Command, Stdio},
    str::FromStr,
};

use glam::{Quat, Vec3, vec3};
use serde::{Deserialize, Serialize};
use stardust_xr_asteroids::{
    ClientState, Context, CustomElement, Migrate, Reify, Tasker, Transformable,
    client::run,
    elements::{Grabbable, Lines, Spatial, Text},
};
use stardust_xr_fusion::{
    drawable::{Line, LinePoint},
    fields::{CylinderShape, Shape},
    node::NodeType,
    root::RootAspect,
    spatial::{SpatialRef, Transform},
    values::color::rgba_linear,
};

use crate::{
    derezzer::Derezzer, spatial_ref::ExternalSpatialRef, spatial_ref_exposer::SpatialRefExposer,
};

#[tokio::main]
async fn main() {
    run::<PalmLauncher>(&[]).await;
}
#[derive(Debug, Serialize, Deserialize, Default)]
enum Action {
    #[default]
    Nothing,
    Command(String),
    Destroy,
}
#[derive(Debug, Serialize, Deserialize, Default)]
enum Target {
    #[default]
    HandLeft,
    HandRight,
    ControllerLeft,
    ControllerRight,
}
impl FromStr for Target {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Hand/Left" => Ok(Self::HandLeft),
            "Hand/Right" => Ok(Self::HandRight),
            "Controller/Left" => Ok(Self::ControllerLeft),
            "Controller/Right" => Ok(Self::ControllerRight),
            _ => Err(s.to_string()),
        }
    }
}
impl Target {
    fn spatial_ref_info(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            Target::HandLeft => (
                "org.stardustxr.Hands",
                "/org/stardustxr/Hand/left/palm",
                "/org/stardustxr/Hand/left",
            ),
            Target::HandRight => (
                "org.stardustxr.Hands",
                "/org/stardustxr/Hand/right/palm",
                "/org/stardustxr/Hand/right",
            ),
            Target::ControllerLeft => (
                "org.stardustxr.Controllers",
                "/org/stardustxr/Controller/left",
                "/org/stardustxr/Controller/left",
            ),
            Target::ControllerRight => (
                "org.stardustxr.Controllers",
                "/org/stardustxr/Controller/right",
                "/org/stardustxr/Controller/right",
            ),
        }
    }
    fn offset(&self) -> (Vec3, Quat) {
        match self {
            Target::HandLeft => (
                vec3(0.0, -0.02, 0.0),
                Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_z(FRAC_PI_2),
            ),
            Target::HandRight => (
                vec3(0.0, -0.02, 0.0),
                Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_z(FRAC_PI_2),
            ),
            Target::ControllerLeft => (vec3(0.0, -0.01, 0.01), Quat::from_rotation_x(-FRAC_PI_3)),
            Target::ControllerRight => todo!(),
        }
    }
    fn text_rot(&self) -> Quat {
        match self {
            Target::HandLeft => Quat::from_rotation_z(FRAC_PI_2),
            Target::HandRight => Quat::from_rotation_z(-FRAC_PI_2) * Quat::from_rotation_x(PI),
            Target::ControllerLeft => Quat::from_rotation_z(FRAC_PI_2),
            Target::ControllerRight => todo!(),
        }
    }
}
#[derive(Debug, Serialize, Deserialize, Default)]
struct PalmLauncher {
    target: Target,
    pos: Vec3,
    rot: Quat,
    state: Action,
    #[serde(skip)]
    handle_ref: Option<SpatialRef>,
    commands: Vec<String>,
    visible: bool,
}

impl Reify for PalmLauncher {
    fn reify(
        &self,
        context: &Context,
        _tasks: impl Tasker<Self>,
    ) -> impl stardust_xr_asteroids::Element<Self> {
        let (name, spatial_path, tracked_path) = self.target.spatial_ref_info();
        let (pos, rot) = self.target.offset();
        ExternalSpatialRef::new(name, spatial_path, Some(tracked_path))
            .tracked_changed(|state: &mut PalmLauncher, tracked| {
                state.visible = tracked;
                state.pos = Vec3::ZERO;
                state.rot = Quat::IDENTITY;
                state.state = Action::Nothing;
            })
            .build()
            .maybe_child(self.visible.then(|| {
                Spatial::default()
                    .pos(pos)
                    .rot(rot)
                    .build()
                    .child(
                        Lines::new([Line {
                            points: {
                                let color = match &self.state {
                                    Action::Nothing => {
                                        context.accent_color.color()
                                            * rgba_linear!(0.1, 0.1, 0.1, 1.0)
                                    }
                                    Action::Command(_) => {
                                        context.accent_color.color()
                                            * rgba_linear!(0.8, 0.8, 0.8, 1.0)
                                    }
                                    Action::Destroy => rgba_linear!(1., 0., 0., 1.),
                                };
                                vec![
                                    LinePoint {
                                        point: Vec3::ZERO.into(),
                                        thickness: 0.001,
                                        color,
                                    },
                                    LinePoint {
                                        point: self.pos.into(),
                                        thickness: 0.001,
                                        color,
                                    },
                                ]
                            },
                            cyclic: false,
                        }])
                        .build(),
                    )
                    .maybe_child(if let Action::Command(cmd) = &self.state {
                        let quat = Quat::from_rotation_arc(Vec3::Y, self.pos.normalize())
                            * self.target.text_rot();
                        Some(
                            Text::new(cmd)
                                .rot(quat)
                                .pos((self.pos * 0.5) + (quat.mul_vec3(Vec3::Y * 0.01)))
                                .build(),
                        )
                    } else {
                        None
                    })
                    .child(
                        SpatialRefExposer::new(|state: &mut Self, spatial_ref| {
                            state.handle_ref = Some(spatial_ref)
                        })
                        .build(),
                    )
                    .maybe_child(matches!(self.state, Action::Destroy).then(|| {
                        Derezzer::new(
                            Vec3::ZERO,
                            Quat::from_rotation_arc(Vec3::Y, self.pos.normalize()),
                            self.pos.length(),
                        )
                        .build()
                    }))
                    .child(
                        Grabbable::new(
                            Shape::Cylinder(CylinderShape {
                                length: 0.02,
                                radius: 0.002,
                            }),
                            self.pos,
                            self.rot,
                            |state: &mut PalmLauncher, pos, rot| {
                                state.pos = pos.into();
                                state.rot = rot.into()
                            },
                        )
                        .max_distance(0.025)
                        .reparentable(false)
                        .grab_stop(|state: &mut PalmLauncher| {
                            if let Action::Command(cmd) = &state.state {
                                let cmd = cmd.clone();
                                let spatial_ref = state.handle_ref.clone().unwrap();
                                let pos = state.pos;
                                tokio::spawn(async move {
                                    let root = spatial_ref.client().get_root();
                                    let quat = Quat::from_rotation_arc(Vec3::Y, pos.normalize())
                                        * Quat::from_rotation_z(FRAC_PI_2);
                                    let spatial = stardust_xr_fusion::spatial::Spatial::create(
                                        &spatial_ref,
                                        Transform::from_translation_rotation(pos * 0.5, quat),
                                    )
                                    .unwrap();
                                    let token = root
                                        .generate_state_token(
                                            stardust_xr_fusion::root::ClientState::from_root(
                                                &spatial,
                                            )
                                            .unwrap(),
                                        )
                                        .await
                                        .unwrap();
                                    Command::new("sh")
                                        .arg("-c")
                                        .env("STARDUST_STARTUP_TOKEN", token)
                                        .arg(format!("{cmd} &"))
                                        .stdin(Stdio::null())
                                        .stdout(Stdio::null())
                                        .stderr(Stdio::null())
                                        .spawn()
                                        .unwrap();
                                });
                            }
                            state.pos = Vec3::ZERO;
                            state.rot = Quat::IDENTITY;
                            state.state = Action::Nothing;
                        })
                        .build()
                        .child(
                            Lines::new([Line {
                                points: vec![
                                    LinePoint {
                                        point: vec3(0.0, -0.01, 0.0).into(),
                                        thickness: 0.002,
                                        color: rgba_linear!(1., 1., 1., 1.),
                                    },
                                    LinePoint {
                                        point: vec3(0.0, 0.01, 0.0).into(),
                                        thickness: 0.002,
                                        color: rgba_linear!(1., 1., 1., 1.),
                                    },
                                ],
                                cyclic: false,
                            }])
                            .build(),
                        ),
                    )
            }))
    }
}
impl ClientState for PalmLauncher {
    const APP_ID: &'static str = "dev.schmarni.palmlauncher";

    fn initial_state_update(&mut self) {
        let mut args = env::args().into_iter().skip(1);
        let target = Target::from_str(
            &args
                .next()
                .expect("no target specified, use Hand/Left Conroller/Right etc"),
        )
        .expect("invalid_target_specified, use Hand/Left Conroller/Right etc");
        let args = args.collect();
        self.target = target;
        self.commands = args;
    }

    fn on_frame(&mut self, _info: &stardust_xr_fusion::root::FrameInfo) {
        let v = 0.5 / self.commands.len() as f32;
        let index = (self.pos.length() / v).floor() as usize;
        self.state = if index == 0 {
            Action::Nothing
        } else if index > self.commands.len() {
            Action::Destroy
        } else {
            let index = index - 1;
            Action::Command(self.commands[index].clone())
        }
    }
}
impl Migrate for PalmLauncher {
    type Old = Self;
}
