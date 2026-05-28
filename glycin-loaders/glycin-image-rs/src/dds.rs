//! Custom DDS (DirectDraw Surface) decoder.
//!
//! The `image` crate only supports BC1/BC2/BC3 (DXT1/DXT3/DXT5). This module
//! adds support for BC4, BC5, BC6H and BC7 as well, by parsing DDS headers
//! locally and dispatching block decoding to the `bcdec_rs` crate.

use std::io::Read;

use image::error::{
    DecodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind,
};
use image::{ColorType, ImageDecoder, ImageError, ImageFormat, ImageResult};

const DDS_MAGIC: &[u8; 4] = b"DDS ";
const HEADER_SIZE: u32 = 124;
const PIXEL_FORMAT_SIZE: u32 = 32;
const PF_FOURCC: u32 = 0x4;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum DdsError {
    Signature,
    HeaderSize(u32),
    HeaderFlags(u32),
    PixelFormatSize(u32),
    DxgiFormat(u32),
    ResourceDimension(u32),
    Dx10Flags(u32),
    Dx10ArraySize(u32),
    DimensionsInvalid,
}

impl std::fmt::Display for DdsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DdsError::Signature => f.write_str("DDS signature not found"),
            DdsError::HeaderSize(s) => write!(f, "Invalid DDS header size: {s}"),
            DdsError::HeaderFlags(fs) => write!(f, "Invalid DDS header flags: {fs:#010X}"),
            DdsError::PixelFormatSize(s) => write!(f, "Invalid DDS PixelFormat size: {s}"),
            DdsError::DxgiFormat(d) => write!(f, "Invalid DDS DXGI format: {d}"),
            DdsError::ResourceDimension(d) => write!(f, "Invalid DDS resource dimension: {d}"),
            DdsError::Dx10Flags(fs) => write!(f, "Invalid DDS DX10 header flags: {fs:#010X}"),
            DdsError::Dx10ArraySize(s) => write!(f, "Invalid DDS DX10 array size: {s}"),
            DdsError::DimensionsInvalid => f.write_str("DDS image dimensions invalid or too large"),
        }
    }
}

impl std::error::Error for DdsError {}

fn dec_err(e: DdsError) -> ImageError {
    ImageError::Decoding(DecodingError::new(ImageFormat::Dds.into(), e))
}

#[derive(Debug, Copy, Clone)]
enum BcFormat {
    Bc1,
    Bc2,
    Bc3,
    Bc4 { signed: bool },
    Bc5 { signed: bool },
    Bc6h { signed: bool },
    Bc7,
}

impl BcFormat {
    fn block_bytes(self) -> usize {
        match self {
            BcFormat::Bc1 | BcFormat::Bc4 { .. } => 8,
            _ => 16,
        }
    }

    fn color_type(self) -> ColorType {
        match self {
            BcFormat::Bc4 { .. } => ColorType::L8,
            BcFormat::Bc5 { .. } => ColorType::Rgb8,
            BcFormat::Bc6h { .. } => ColorType::Rgb32F,
            _ => ColorType::Rgba8,
        }
    }

    fn out_bytes_per_pixel(self) -> usize {
        match self.color_type() {
            ColorType::L8 => 1,
            ColorType::Rgb8 => 3,
            ColorType::Rgba8 => 4,
            ColorType::Rgb32F => 12,
            _ => unreachable!(),
        }
    }
}

fn read_u32_le(r: &mut dyn Read) -> ImageResult<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn skip(r: &mut dyn Read, n: usize) -> ImageResult<()> {
    let mut buf = [0u8; 64];
    let mut left = n;
    while left > 0 {
        let take = left.min(buf.len());
        r.read_exact(&mut buf[..take])?;
        left -= take;
    }
    Ok(())
}

/// DDS decoder that supports all BCn compressed formats (BC1-BC7).
pub struct DdsDecoder {
    width: u32,
    height: u32,
    format: BcFormat,
    payload: Vec<u8>,
}

impl DdsDecoder {
    pub fn new<R: Read>(mut r: R) -> ImageResult<Self> {
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if &magic != DDS_MAGIC {
            return Err(dec_err(DdsError::Signature));
        }

        let header_size = read_u32_le(&mut r)?;
        if header_size != HEADER_SIZE {
            return Err(dec_err(DdsError::HeaderSize(header_size)));
        }

        const REQUIRED_FLAGS: u32 = 0x1 | 0x2 | 0x4 | 0x1000;
        const VALID_FLAGS: u32 =
            0x1 | 0x2 | 0x4 | 0x8 | 0x1000 | 0x20000 | 0x80000 | 0x0080_0000;
        let flags = read_u32_le(&mut r)?;
        if flags & (REQUIRED_FLAGS | !VALID_FLAGS) != REQUIRED_FLAGS {
            return Err(dec_err(DdsError::HeaderFlags(flags)));
        }

        let height = read_u32_le(&mut r)?;
        let width = read_u32_le(&mut r)?;
        let _pitch_or_linear_size = read_u32_le(&mut r)?;
        let _depth = read_u32_le(&mut r)?;
        let _mipmap_count = read_u32_le(&mut r)?;
        skip(&mut r, 4 * 11)?; // dwReserved1

        let pf_size = read_u32_le(&mut r)?;
        if pf_size != PIXEL_FORMAT_SIZE {
            return Err(dec_err(DdsError::PixelFormatSize(pf_size)));
        }
        let pf_flags = read_u32_le(&mut r)?;
        let mut fourcc = [0u8; 4];
        r.read_exact(&mut fourcc)?;
        let _rgb_bit_count = read_u32_le(&mut r)?;
        let _r_mask = read_u32_le(&mut r)?;
        let _g_mask = read_u32_le(&mut r)?;
        let _b_mask = read_u32_le(&mut r)?;
        let _a_mask = read_u32_le(&mut r)?;
        skip(&mut r, 4 * 5)?; // caps[4] + dwReserved2

        if pf_flags & PF_FOURCC == 0 {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Dds.into(),
                    UnsupportedErrorKind::Format(ImageFormatHint::Name(
                        "DDS (uncompressed)".to_string(),
                    )),
                ),
            ));
        }

        let format = match &fourcc {
            b"DXT1" => BcFormat::Bc1,
            b"DXT2" | b"DXT3" => BcFormat::Bc2,
            b"DXT4" | b"DXT5" => BcFormat::Bc3,
            b"ATI1" | b"BC4U" => BcFormat::Bc4 { signed: false },
            b"BC4S" => BcFormat::Bc4 { signed: true },
            b"ATI2" | b"BC5U" => BcFormat::Bc5 { signed: false },
            b"BC5S" => BcFormat::Bc5 { signed: true },
            b"DX10" => read_dx10_format(&mut r)?,
            _ => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Dds.into(),
                        UnsupportedErrorKind::GenericFeature(format!(
                            "DDS FourCC {fourcc:?}"
                        )),
                    ),
                ));
            }
        };

        if width == 0 || height == 0 {
            return Err(dec_err(DdsError::DimensionsInvalid));
        }

        let bpp = format.out_bytes_per_pixel();
        let total = (width as usize)
            .checked_mul(height as usize)
            .and_then(|v| v.checked_mul(bpp));
        if total.is_none() {
            return Err(dec_err(DdsError::DimensionsInvalid));
        }

        let bw = width.div_ceil(4) as usize;
        let bh = height.div_ceil(4) as usize;
        let n_blocks = bw
            .checked_mul(bh)
            .ok_or_else(|| dec_err(DdsError::DimensionsInvalid))?;
        let needed = n_blocks
            .checked_mul(format.block_bytes())
            .ok_or_else(|| dec_err(DdsError::DimensionsInvalid))?;

        let mut payload = vec![0u8; needed];
        r.read_exact(&mut payload)?;

        Ok(Self {
            width,
            height,
            format,
            payload,
        })
    }
}

fn read_dx10_format(r: &mut dyn Read) -> ImageResult<BcFormat> {
    let dxgi_format = read_u32_le(r)?;
    let resource_dimension = read_u32_le(r)?;
    let misc_flag = read_u32_le(r)?;
    let array_size = read_u32_le(r)?;
    let misc_flags_2 = read_u32_le(r)?;

    if dxgi_format > 132 {
        return Err(dec_err(DdsError::DxgiFormat(dxgi_format)));
    }
    if !(2..=4).contains(&resource_dimension) {
        return Err(dec_err(DdsError::ResourceDimension(resource_dimension)));
    }
    if misc_flag != 0x0 && misc_flag != 0x4 {
        return Err(dec_err(DdsError::Dx10Flags(misc_flag)));
    }
    if resource_dimension == 4 && array_size != 1 {
        return Err(dec_err(DdsError::Dx10ArraySize(array_size)));
    }
    if misc_flags_2 > 0x4 {
        return Err(dec_err(DdsError::Dx10Flags(misc_flags_2)));
    }

    // DXGI format numbers per
    // https://learn.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format
    Ok(match dxgi_format {
        70..=72 => BcFormat::Bc1,
        73..=75 => BcFormat::Bc2,
        76..=78 => BcFormat::Bc3,
        79 | 80 => BcFormat::Bc4 { signed: false },
        81 => BcFormat::Bc4 { signed: true },
        82 | 83 => BcFormat::Bc5 { signed: false },
        84 => BcFormat::Bc5 { signed: true },
        94 | 95 => BcFormat::Bc6h { signed: false },
        96 => BcFormat::Bc6h { signed: true },
        97..=99 => BcFormat::Bc7,
        _ => {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Dds.into(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "DDS DXGI Format {dxgi_format}"
                    )),
                ),
            ));
        }
    })
}

impl ImageDecoder for DdsDecoder {
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn color_type(&self) -> ColorType {
        self.format.color_type()
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        decode_into(self.width, self.height, self.format, &self.payload, buf)
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

fn decode_into(
    width: u32,
    height: u32,
    format: BcFormat,
    payload: &[u8],
    out: &mut [u8],
) -> ImageResult<()> {
    let w = width as usize;
    let h = height as usize;
    let bw = width.div_ceil(4) as usize;
    let bh = height.div_ceil(4) as usize;
    let block_bytes = format.block_bytes();

    match format {
        BcFormat::Bc1 | BcFormat::Bc2 | BcFormat::Bc3 | BcFormat::Bc7 => {
            let mut block_out = [0u8; 4 * 4 * 4];
            for by in 0..bh {
                for bx in 0..bw {
                    let start = (by * bw + bx) * block_bytes;
                    let block_in = &payload[start..start + block_bytes];
                    match format {
                        BcFormat::Bc1 => bcdec_rs::bc1(block_in, &mut block_out, 4 * 4),
                        BcFormat::Bc2 => bcdec_rs::bc2(block_in, &mut block_out, 4 * 4),
                        BcFormat::Bc3 => bcdec_rs::bc3(block_in, &mut block_out, 4 * 4),
                        BcFormat::Bc7 => bcdec_rs::bc7(block_in, &mut block_out, 4 * 4),
                        _ => unreachable!(),
                    }
                    let rows = (h - by * 4).min(4);
                    let cols = (w - bx * 4).min(4);
                    for row in 0..rows {
                        let dst = ((by * 4 + row) * w + bx * 4) * 4;
                        let src = row * 4 * 4;
                        out[dst..dst + cols * 4]
                            .copy_from_slice(&block_out[src..src + cols * 4]);
                    }
                }
            }
        }
        BcFormat::Bc4 { signed } => {
            let mut block_out = [0u8; 4 * 4];
            for by in 0..bh {
                for bx in 0..bw {
                    let start = (by * bw + bx) * block_bytes;
                    let block_in = &payload[start..start + block_bytes];
                    bcdec_rs::bc4(block_in, &mut block_out, 4, signed);
                    let rows = (h - by * 4).min(4);
                    let cols = (w - bx * 4).min(4);
                    for row in 0..rows {
                        let dst = (by * 4 + row) * w + bx * 4;
                        let src = row * 4;
                        out[dst..dst + cols]
                            .copy_from_slice(&block_out[src..src + cols]);
                    }
                }
            }
        }
        BcFormat::Bc5 { signed } => {
            // bcdec_rs::bc5 writes RG (2 bytes per pixel); expand to RGB with B=0.
            let mut block_out = [0u8; 4 * 4 * 2];
            for by in 0..bh {
                for bx in 0..bw {
                    let start = (by * bw + bx) * block_bytes;
                    let block_in = &payload[start..start + block_bytes];
                    bcdec_rs::bc5(block_in, &mut block_out, 4 * 2, signed);
                    let rows = (h - by * 4).min(4);
                    let cols = (w - bx * 4).min(4);
                    for row in 0..rows {
                        for col in 0..cols {
                            let dst = ((by * 4 + row) * w + bx * 4 + col) * 3;
                            let src = (row * 4 + col) * 2;
                            out[dst] = block_out[src];
                            out[dst + 1] = block_out[src + 1];
                            out[dst + 2] = 0;
                        }
                    }
                }
            }
        }
        BcFormat::Bc6h { signed } => {
            // RGB float32; write native-endian bytes.
            let mut block_out = [0f32; 4 * 4 * 3];
            for by in 0..bh {
                for bx in 0..bw {
                    let start = (by * bw + bx) * block_bytes;
                    let block_in = &payload[start..start + block_bytes];
                    bcdec_rs::bc6h_float(block_in, &mut block_out, 4 * 3, signed);
                    let rows = (h - by * 4).min(4);
                    let cols = (w - bx * 4).min(4);
                    for row in 0..rows {
                        for col in 0..cols {
                            let dst = ((by * 4 + row) * w + bx * 4 + col) * 12;
                            let src = (row * 4 + col) * 3;
                            for k in 0..3 {
                                let bytes = block_out[src + k].to_ne_bytes();
                                out[dst + k * 4..dst + k * 4 + 4]
                                    .copy_from_slice(&bytes);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
