use std::time::Duration;
use std::{io::ErrorKind, time::Instant};
use std::io::Error;
use sysinfo::System;
use f16_hid::{
    Bitmap8, Command, LedMatrix, DISPLAY_HEIGHT, DISPLAY_WIDTH
};

const BG_VALUE: u8 = 2;
const ERROR_RETRY_PAUSE: Duration = Duration::from_secs(2);

fn main() {
    // TODO: Handle finding device names

    let mut matrix_left = LedMatrix::new("/dev/ttyACM0")
        .expect("Unable to open port");
    let mut matrix_right = LedMatrix::new("/dev/ttyACM1")
        .expect("Unable to open port");

    let mut start;
    let mut sys = System::new();
    let mut image = Bitmap8::new();

    let command = Command::Brightness(0xff);
    matrix_left.execute(command.clone()).expect("Command failed");
    matrix_right.execute(command).expect("Command failed");

    loop {
        start = Instant::now();

        // Refreshing CPU information. This takes time so there's a sleep at
        // end of this loop to take up the slack
        sys.refresh_cpu(); 

        let mut cpu_values: Vec<u8> = sys.cpus().iter().map(|x| x.cpu_usage() as u8).collect();

        let mut values: Vec<u8> = cpu_values.drain(0..=7).collect();
        draw_vu_meter(&mut image, values);
        match display_bitmap(&mut matrix_left, &image) {
            Err(result) => handle_serial_error(result, &mut matrix_left),
            _ => {}
        }

        values = cpu_values.drain(0..=7).collect();
        draw_vu_meter(&mut image, values);
        match display_bitmap(&mut matrix_right, &image) {
            Err(result) => handle_serial_error(result, &mut matrix_right),
            _ => {}
        }
        let remaining_time = Instant::now() - start;

        // If there's time left over after updatind displays, sleep the
        // rest of thd time
        if remaining_time < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL - remaining_time);
        }
    }
}

fn handle_serial_error(error: Error, matrix: &mut LedMatrix) {
    match error.kind() {
        ErrorKind::TimedOut => {
            eprintln!("Timed out, safe to retry");
        },
        ErrorKind::BrokenPipe => {
            eprintln!("Broken, need to re-connect");

            // This can fail, but we don't care for now
            let result = matrix.reconnect();

            if result.is_err() {
                eprintln!("Unable to reconnect port {}. Error: {:?}", matrix.path(), result);
            }
        },
        _ => {
            panic!("Something new happened, unsure how to recorver {:?}", error);
        }
    }

    // If any error happened, wait to try again
    std::thread::sleep(ERROR_RETRY_PAUSE);
}

/// Stage VU meter in a bitmap buffer
fn draw_vu_meter(bitmap: &mut Bitmap8, values: Vec<u8>) {
    bitmap.fill(BG_VALUE);
    bitmap.draw_box(0, DISPLAY_HEIGHT - 20, DISPLAY_WIDTH - 1, DISPLAY_HEIGHT - 1, 0);
    bitmap.draw_box(0, DISPLAY_HEIGHT - 19, DISPLAY_WIDTH - 1, DISPLAY_HEIGHT - 2, BG_VALUE);
    bitmap.draw_box(DISPLAY_WIDTH / 2, DISPLAY_HEIGHT - 19, DISPLAY_WIDTH / 2, DISPLAY_HEIGHT - 2, 0);

    for (mut index, value) in values.iter().enumerate() {
        let value = value.clone() as usize;
        let col_start = DISPLAY_HEIGHT - 2 - ((17 * value) / 100);
        let col_end = DISPLAY_HEIGHT - 2;

        // Skip over the middle. This is *all yucky*
        if index > 3 {
            index += 1;
        }

        bitmap.draw_box(index, col_start, index, col_end, 20);
    }
}

/// Send bitmap to display
fn display_bitmap(matrix: &mut LedMatrix, bitmap: &Bitmap8) -> Result<(), Error> {
    for y in 0 .. DISPLAY_WIDTH {
        let col_start = y * DISPLAY_HEIGHT;
        let col_end = col_start + DISPLAY_HEIGHT;

        let command = Command::StageColumnBuffer((y as u8, &bitmap.data()[col_start..col_end]));
        matrix.execute(command)?;
    }

    let command = Command::DrawBuffer;
    matrix.execute(command)?;

    Ok(())
}

