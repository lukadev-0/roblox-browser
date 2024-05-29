use std::io::{Read, Write};

use anyhow::anyhow;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChunkPos {
    pub x: u8,
    pub y: u8,
}

impl ChunkPos {
    pub fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), anyhow::Error> {
        writer.write_u8(self.x)?;
        writer.write_u8(self.y)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ClientCommand {
    Reset,
    Load { url: String },
    Mouse { x: u32, y: u32, event: MouseEvent },
}

#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    Move,
    Pressed(MouseButton),
    Released(MouseButton),
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
}

impl From<MouseEvent> for u8 {
    fn from(value: MouseEvent) -> Self {
        match value {
            MouseEvent::Move => 0,
            MouseEvent::Pressed(MouseButton::Left) => 1,
            MouseEvent::Pressed(MouseButton::Right) => 2,
            MouseEvent::Released(MouseButton::Left) => 3,
            MouseEvent::Released(MouseButton::Right) => 4,
        }
    }
}

impl TryFrom<u8> for MouseEvent {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MouseEvent::Move),
            1 => Ok(MouseEvent::Pressed(MouseButton::Left)),
            2 => Ok(MouseEvent::Pressed(MouseButton::Right)),
            3 => Ok(MouseEvent::Released(MouseButton::Left)),
            4 => Ok(MouseEvent::Released(MouseButton::Right)),
            _ => Err(anyhow!("invalid mouse event: {}", value)),
        }
    }
}

impl ClientCommand {
    pub fn read(reader: &mut impl Read) -> Result<Self, anyhow::Error> {
        let op = reader.read_u8()?;
        match op {
            0 => Ok(ClientCommand::Reset),
            1 => {
                let len = reader.read_u32::<LittleEndian>()?;
                let mut url = vec![0; len as usize];
                reader.read_exact(&mut url)?;
                let url = String::from_utf8(url)?;

                Ok(ClientCommand::Load { url })
            }
            2 => {
                let x = reader.read_u32::<LittleEndian>()?;
                let y = reader.read_u32::<LittleEndian>()?;
                let event = reader.read_u8()?.try_into()?;

                Ok(ClientCommand::Mouse { x, y, event })
            }
            _ => Err(anyhow!("invalid command: {}", op)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ServerCommand {
    Resize { width: u32, height: u32 },
    ChunkData { chunk_pos: ChunkPos, data: Bytes },
}

impl ServerCommand {
    pub fn write(&self, writer: &mut impl Write) -> Result<(), anyhow::Error> {
        match self {
            ServerCommand::Resize { width, height } => {
                writer.write_u8(0)?;
                writer.write_u32::<LittleEndian>(*width)?;
                writer.write_u32::<LittleEndian>(*height)?;
            }
            ServerCommand::ChunkData { chunk_pos, data } => {
                writer.write_u8(1)?;
                chunk_pos.write(writer)?;
                writer.write_u32::<LittleEndian>(data.len() as u32)?;
                writer.write_all(data)?;
            }
        }
        Ok(())
    }
}
