#![allow(dead_code)]

use std::{io::BufRead, result::Result};

use nom::{combinator::all_consuming, sequence::tuple, IResult};

use kioku::Arena;

use data_tree::{reader::DataTreeReader, Event};

use crate::{
    light::{DistantDiskLight, RectangleLight, SphereLight},
    math::Vector,
};

use super::{
    basics::ws_f32,
    psy::{parse_color, PsyParseError},
};

pub fn parse_distant_disk_light<'a>(
    arena: &'a Arena,
    events: &mut DataTreeReader<impl BufRead>,
    _ident: Option<&str>,
) -> Result<DistantDiskLight<'a>, PsyParseError> {
    let mut radii = Vec::new();
    let mut directions = Vec::new();
    let mut colors = Vec::new();

    // Parse
    loop {
        match events.next_event()? {
            Event::Leaf {
                type_name: "Radius",
                contents,
                byte_offset,
            } => {
                if let IResult::Ok((_, radius)) = all_consuming(ws_f32)(&contents) {
                    radii.push(radius);
                } else {
                    // Found radius, but its contents is not in the right format
                    return Err(PsyParseError::UnknownError(byte_offset));
                }
            }

            // Direction
            Event::Leaf {
                type_name: "Direction",
                contents,
                byte_offset,
            } => {
                if let IResult::Ok((_, direction)) =
                    all_consuming(tuple((ws_f32, ws_f32, ws_f32)))(&contents)
                {
                    directions.push(Vector::new(direction.0, direction.1, direction.2));
                } else {
                    // Found direction, but its contents is not in the right format
                    return Err(PsyParseError::UnknownError(byte_offset));
                }
            }

            // Color
            Event::Leaf {
                type_name: "Color",
                contents,
                byte_offset,
            } => {
                if let Ok(color) = parse_color(&contents) {
                    colors.push(color);
                } else {
                    // Found color, but its contents is not in the right format
                    return Err(PsyParseError::UnknownError(byte_offset));
                }
            }

            Event::InnerClose { .. } => {
                break;
            }

            _ => {
                todo!(); // Return error.
            }
        }
    }

    return Ok(DistantDiskLight::new(arena, &radii, &directions, &colors));
}

pub fn parse_sphere_light<'a>(
    arena: &'a Arena,
    events: &mut DataTreeReader<impl BufRead>,
) -> Result<SphereLight<'a>, PsyParseError> {
    let mut radii = Vec::new();
    let mut colors = Vec::new();

    // Parse
    loop {
        match events.next_event()? {
            // Radius
            Event::Leaf {
                type_name: "Radius",
                contents,
                byte_offset,
            } => {
                if let IResult::Ok((_, radius)) = all_consuming(ws_f32)(&contents) {
                    radii.push(radius);
                } else {
                    // Found radius, but its contents is not in the right format
                    return Err(PsyParseError::UnknownError(byte_offset));
                }
            }

            // Color
            Event::Leaf {
                type_name: "Color",
                contents,
                byte_offset,
            } => {
                if let Ok(color) = parse_color(&contents) {
                    colors.push(color);
                } else {
                    // Found color, but its contents is not in the right format
                    return Err(PsyParseError::UnknownError(byte_offset));
                }
            }

            Event::InnerClose { .. } => {
                break;
            }

            _ => {
                todo!(); // Return error.
            }
        }
    }

    return Ok(SphereLight::new(arena, &radii, &colors));
}

pub fn parse_rectangle_light<'a>(
    arena: &'a Arena,
    events: &mut DataTreeReader<impl BufRead>,
) -> Result<RectangleLight<'a>, PsyParseError> {
    let mut dimensions = Vec::new();
    let mut colors = Vec::new();

    // Parse
    loop {
        match events.next_event()? {
            // Dimensions
            Event::Leaf {
                type_name: "Dimensions",
                contents,
                byte_offset,
            } => {
                if let IResult::Ok((_, radius)) = all_consuming(tuple((ws_f32, ws_f32)))(&contents)
                {
                    dimensions.push(radius);
                } else {
                    // Found dimensions, but its contents is not in the right format
                    return Err(PsyParseError::UnknownError(byte_offset));
                }
            }

            // Color
            Event::Leaf {
                type_name: "Color",
                contents,
                byte_offset,
            } => {
                if let Ok(color) = parse_color(&contents) {
                    colors.push(color);
                } else {
                    // Found color, but its contents is not in the right format
                    return Err(PsyParseError::UnknownError(byte_offset));
                }
            }

            Event::InnerClose { .. } => {
                break;
            }

            _ => {
                todo!(); // Return error.
            }
        }
    }

    return Ok(RectangleLight::new(arena, &dimensions, &colors));
}
