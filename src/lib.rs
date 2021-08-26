use log::trace;
use std::time::Duration;

pub struct FrameContext<P: image::Pixel> {
    pub current: usize,
    pub limits: usize,
    pub timestamp: Duration,
    pub frame: image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>>,
    pub width: u32,
    pub height: u32,
}

impl<P: 'static + image::Pixel> FrameContext<P>
where
    P: image::Pixel + std::cmp::PartialEq,
{
    pub fn new(
        timestamp: Duration,
        limits: usize,
        frame: image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>>,
    ) -> Self {
        FrameContext {
            current: 0,
            limits,
            timestamp,
            width: frame.width(),
            height: frame.height(),
            frame,
        }
    }
}

pub struct PartialFrame<P: image::Pixel> {
    pub x: u32,
    pub y: u32,
    pub image: image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>>,
}

pub enum Frame<P: image::Pixel> {
    KeyFrame(image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>>),
    PartialFrame(Vec<PartialFrame<P>>),
}

impl<P: 'static + image::Pixel> FrameContext<P>
where
    P: image::Pixel + std::cmp::PartialEq,
{
    pub fn push(
        &mut self,
        timestamp: &Duration,
        frame: image::ImageBuffer<P, Vec<<P as image::Pixel>::Subpixel>>,
    ) -> Frame<P> {
        if self.current < self.limits {
            self.current += 1;
            let mut frames = Vec::new();
            const BLOCK_SIZE: usize = 16;
            const BIT_VALUE: u16 = 1;
            #[inline(always)]
            fn print_bits_ln(val: &u16) -> String {
                format!("{:#018b}\n", val)
            }
            #[inline(always)]
            fn round_to_size<const N: usize>(len: usize) -> usize {
                ((len + (N - 1)) & !(N - 1)) / N
            }
            let res = imageproc::utils::pixel_diffs(&frame, &self.frame, |p, q| p != q);
            let mut bit_map = [0; BLOCK_SIZE];
            let y_base = round_to_size::<BLOCK_SIZE>(self.height as usize);
            let x_base = round_to_size::<BLOCK_SIZE>(self.width as usize);
            for diff in &res {
                let bit = BIT_VALUE << ((BLOCK_SIZE - 1) - (diff.x as usize / x_base));
                bit_map[diff.y as usize / y_base] |= bit;
            }

            let mut dump = format!(
                "{}x{}------------------------------------------------------\n",
                self.width, self.height
            );
            for bits in bit_map.iter() {
                dump.push_str(&print_bits_ln(bits));
            }
            dump.push_str("------------------------------------------------------");
            trace!("{}", dump);
            for y_idx in 0..bit_map.len() {
                for x_idx in 0..BLOCK_SIZE {
                    let bits = bit_map[y_idx].reverse_bits();
                    if bits == 0 {
                        continue;
                    }
                    if (bits & (BIT_VALUE << x_idx)) != 0 {
                        let x = (x_idx * x_base) as u32;
                        let y = (y_idx * y_base) as u32;
                        let width = x_base as u32;
                        let height = y_base as u32;
                        let sub_image = image::imageops::crop_imm(&frame, x, y, width, height);
                        let image = sub_image.to_image();
                        frames.push(PartialFrame {
                            x,
                            y,
                            image: image.clone(),
                        });
                    }
                }
            }
            self.timestamp = timestamp.clone();
            self.frame = frame.clone();
            Frame::PartialFrame(frames)
        } else {
            self.current = 0;
            self.timestamp = timestamp.clone();
            self.frame = frame.clone();
            Frame::KeyFrame(frame)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::FrameContext;
    use std::time::Duration;

    #[test]
    fn it_works() -> image::ImageResult<()> {
        use image::io::Reader as ImageReader;

        let img = ImageReader::open("./tests/black.png")?.decode()?;
        let img2 = ImageReader::open("./tests/5dot.png")?.decode()?;

        let mut ctx = FrameContext::new(Duration::from_secs(1), 10, img.to_rgb8());

        match ctx.push(&Duration::from_secs(2), img2.to_rgb8()) {
            crate::Frame::KeyFrame(frame) => {
                println!("{:?}", frame);
            }
            crate::Frame::PartialFrame(frames) => {
                assert_eq!(frames.len(), 5);
                for frame in frames {
                    println!(
                        "{},{} {}x{}",
                        frame.x,
                        frame.y,
                        frame.image.width(),
                        frame.image.height()
                    );
                    frame.image.save_with_format(
                        format!("./tests/partial_{}_{}.jpg", frame.x, frame.y),
                        image::ImageFormat::Jpeg,
                    )?;
                }
            }
        }
        Ok(())
    }
}
