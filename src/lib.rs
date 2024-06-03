use std::time::Duration;
use serialport::SerialPort;

pub const DRAW_COMMAND_LENGTH: usize = 39;
pub const MAX_COMMAND_LENGTH: usize = 42;
pub const DISPLAY_WIDTH: usize = 9;
pub const DISPLAY_HEIGHT: usize = 34;

pub const CONNECT_DELAY: Duration = Duration::from_millis(100);
pub const RECONNECT_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone)]
/// Bitmaps with 8 bits of definition. This is stored rotated 90 degress given
/// that the staging commands are column based. Draw commands will automatically
/// adjust this
pub struct Bitmap8 {
    pub(crate) data: [u8; DISPLAY_HEIGHT * DISPLAY_WIDTH]
}

impl Bitmap8 {
    pub fn new() -> Self {
        Self {
            data: [0u8; DISPLAY_HEIGHT * DISPLAY_WIDTH]
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn fill(&mut self, value: u8) {
        self.data.fill(value)
    }

    pub fn draw_point(&mut self, x: usize, y: usize, value: u8) -> Result<(), &'static str> {
        if x >= DISPLAY_WIDTH {
            return Err("X was too large");
        } else if y >= DISPLAY_HEIGHT {
            return Err("Y was too large");
        }

        let location = x * DISPLAY_HEIGHT + y;

        self.data[location] = value;

        Ok(())
    }

    pub fn draw_box(&mut self, x1: usize, y1: usize, x2: usize, y2: usize, value: u8) {
        // Originally written by ChatGPT with this prompt:
        // "given an image buffer that's rotated 90 degress, create a function that takes x1, y1, x2, and y2 and a value as a u8 and draw a box" 
        // Then fiddled

        let x_min = x1.min(x2);
        let x_max = x1.max(x2);
        let y_min = y1.min(y2);
        let y_max = y1.max(y2);
    
        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let index = x * DISPLAY_HEIGHT + y;
                self.data[index] = value;
            }
        }
    
    }
}


#[derive(Clone)]
pub struct Bitmap {
    pub(crate) data: [u8; DRAW_COMMAND_LENGTH]
}

impl Bitmap {
    pub fn new() -> Self {
        Self {
            data: [0u8; DRAW_COMMAND_LENGTH]
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn fill(&mut self, value: u8) {
        self.data.fill(value)
    }

    pub fn draw_point(&mut self, x: usize, y: usize, value: bool) -> Result<(), &'static str> {
        if x >= DISPLAY_WIDTH {
            return Err("X was too large");
        } else if y >= DISPLAY_HEIGHT {
            return Err("Y was too large");
        }

        let location = y + (x * DISPLAY_HEIGHT);
        let byte_index = location / 8;
        let bitmask = 1 << location % 8;

        if value {
            self.data[byte_index] |= bitmask;
        } else {
            self.data[byte_index] &= bitmask ^ 0xff;
        } 

        Ok(())
    }
}

#[derive(Clone)]
pub enum Patterns {
    Percentage(u8),
    Gradient,
    DoubleGradient,
    DisplayLotus,
    ZigZag,
    FullBrightness,
    DisplayPanic,
    DisplayLotus2,
}

impl Patterns {
    fn pack(self, data: &mut [u8]) {
        match self {
            Self::Percentage(value) => {
                data[0] = 0x00;
                data[1] = value;
                return;
            },
            _ => () 
        }

        data[0] = match self {
            Self::Gradient => 0x01,
            Self::DoubleGradient => 0x02,
            Self::DisplayLotus => 0x03,
            Self::ZigZag => 0x04,
            Self::FullBrightness => 0x05,
            Self::DisplayPanic => 0x06,
            Self::DisplayLotus2 => 0x07,
            _ => panic!("Should never get here")
        };
    }
}


#[derive(Clone)]
/// Commands to execute. Firmware implementation of these commands can be found here:
/// <https://github.com/FrameworkComputer/inputmodule-rs/blob/main/fl16-inputmodules/src/control.rs#L512>
pub enum Command<'a> {
    Brightness(u8),
    Pattern(Patterns),
    Bootloader,
    Sleep(bool),
    Animate,
    Panic,
    Draw(Box<Bitmap>),
    StageColumnBuffer((u8, &'a [u8])),
    DrawBuffer,
    Version
}

impl<'a> Command<'a> {
    // TODO: Return a result. Some of these commands should have a safety net
    fn pack(self, data: &mut [u8]) {
        match self {
            Self::Brightness(x) => {
                data[0] = 0x00;
                data[1] = x;
            },
            Self::Pattern(pattern) => {
                data[0] = 0x01;
                pattern.pack(&mut data[1..3])
            }
            Self::Bootloader => {
                data[0] = 0x02;
            }
            Self::Sleep(value) => {
                data[0] = 0x03;
                data[1] = if value {
                    1
                } else {
                    0
                };
            },
            Self::Animate => {
                data[0] = 0x04
            }
            Self::Panic => {
                data[0] = 0x05
            },
            Self::Draw(bitmap) => {
                data[0] = 0x06;
                data[1..40].copy_from_slice(&bitmap.data);
            },
            Self::StageColumnBuffer((index, value)) => {
                if index as usize > DISPLAY_WIDTH {
                    panic!("Wrong column index")
                }

                data[0] = 0x07;
                data[1] = index;
                data[2..DISPLAY_HEIGHT + 2].copy_from_slice(&*value)
            },
            Self::DrawBuffer => {
                data[0] = 0x08;
            }
            Self::Version => {
                data[0] = 0x20;
            },
        }
    }
}


pub struct LedMatrix<'a> {
    path: &'a str,
    port: Option<Box<dyn SerialPort>>
}

impl<'a> LedMatrix<'a> {
    pub fn new(path: &'a str) -> Result<Self, serialport::Error> {
        let port = serialport::new(path, 115_200)
            .timeout(CONNECT_DELAY)
            .open()?;

        Ok(Self {
            path,
            port: Some(port)
        })
    }

    pub fn reconnect(&mut self) -> Result<(), serialport::Error> {
        // Hopefully this will yeild the port fast enough
        self.port = None;

        self.port = Some(serialport::new(self.path, 115_200)
            .timeout(RECONNECT_DELAY)
            .open()?);

        Ok(())
    }

    pub fn execute(&mut self, command: Command) -> Result<usize, std::io::Error> {
        let mut buffer = [0u8;MAX_COMMAND_LENGTH];

        buffer[0] = 0x32;
        buffer[1] = 0xac;

        command.pack(&mut buffer[2..]);

        match &mut self.port {
            Some(x) => x.write(&buffer),
            // TODO: This should return the correct ErrorKind, but I need Internet. :D
            None => panic!("Attempted to write to the serial port without opening it")
        }
    }

    pub fn path(&self) -> &'a str {
        self.path
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use sysinfo::System;

    #[test]
    fn set_brightness() {
        let mut matrix = LedMatrix::new("/dev/ttyACM0")
            .expect("Unable to open port");

        let command = Command::Brightness(0x40);

        matrix.execute(command).expect("Command failed");
    }

    #[test]
    fn wake() {
        let mut matrix = LedMatrix::new("/dev/ttyACM0")
            .expect("Unable to open port");

        let command = Command::Sleep(false);

        matrix.execute(command).expect("Command failed");
    }

    #[test]
    fn draw() {
        let mut matrix = LedMatrix::new("/dev/ttyACM1")
            .expect("Unable to open port");

        let mut data = [0u8; 39];
        data[0] = 0xac;
        data[1] = 0xac;
        data[2] = 0xac;

        let command = Command::Brightness(0xff);
        matrix.execute(command).expect("Command failed");

        let mut bitmap = Bitmap::new();
        bitmap.draw_point(0, 0, true).unwrap();
        bitmap.draw_point(4, 0, true).unwrap();
        bitmap.draw_point(4, 4, true).unwrap();
        bitmap.draw_point(0, 4, true).unwrap();

        let command = Command::Draw(Box::new(bitmap));
        matrix.execute(command).expect("Command failed");
    }


    #[test]
    fn draw_greyscale() {
        const BG_VALUE: u8 = 2;

        let mut matrix = LedMatrix::new("/dev/ttyACM1")
            .expect("Unable to open port");
        let mut sys = System::new();
        let mut image = Bitmap8::new();

        let command = Command::Brightness(0xff);
        matrix.execute(command).expect("Command failed");

        loop {
            let mut cpus = Vec::new();
            //std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);

            sys.refresh_cpu(); // Refreshing CPU information.
            for cpu in sys.cpus() {
                cpus.push(cpu.cpu_usage() as u8);
            }
        
            image.fill(BG_VALUE);
            image.draw_box(0, DISPLAY_HEIGHT - 20, DISPLAY_WIDTH - 1, DISPLAY_HEIGHT - 1, 0);
            image.draw_box(0, DISPLAY_HEIGHT - 19, DISPLAY_WIDTH - 1, DISPLAY_HEIGHT - 2, BG_VALUE);
            image.draw_box(DISPLAY_WIDTH / 2, DISPLAY_HEIGHT - 19, DISPLAY_WIDTH / 2, DISPLAY_HEIGHT - 2, 0);

            for (mut index, cpu) in sys.cpus().iter().take(8).enumerate() {
                let value = cpu.cpu_usage() as usize;
                let col_start = DISPLAY_HEIGHT - 2 - ((17 * value) / 100);
                let col_end = DISPLAY_HEIGHT - 2;

                // Skip over the middle. This is *all yucky*
                if index > 3 {
                    index += 1;
                }

                image.draw_box(index, col_start, index, col_end, 20);
            }

            for y in 0 .. DISPLAY_WIDTH {
                let col_start = y * DISPLAY_HEIGHT;
                let col_end = col_start + DISPLAY_HEIGHT;

                let command = Command::StageColumnBuffer((y as u8, &image.data[col_start..col_end]));
                matrix.execute(command).expect("Command failed");
            }

            let command = Command::DrawBuffer;
            matrix.execute(command).expect("Command failed");
        }
    }


    #[test]
    fn display_progress() {
        let mut matrix = LedMatrix::new("/dev/ttyACM1")
            .expect("Unable to open port");

        let command = Command::Brightness(25);
        matrix.execute(command).expect("Command failed");

        for index in 0 ..= 100 {
            let command = Command::Pattern(Patterns::Percentage(index));
            matrix.execute(command).expect("Command failed");
        }

        let command = Command::Pattern(Patterns::DisplayLotus2);
        matrix.execute(command).expect("Command failed");    
    }
}
