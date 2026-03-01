use anyhow::{bail, ensure, Context, Result};

const RIFF_HEADER_LEN: usize = 12; // "RIFF" + size + "WAVE"

/// Concatenates multiple WAV segments into a single WAV file.
///
/// All segments must share the same audio format (channels, sample rate,
/// bits per sample). The function extracts PCM data from each segment,
/// combines them, and produces a new WAV with a valid header.
///
/// # Errors
///
/// Returns an error if any segment is malformed or formats are inconsistent.
pub fn concatenate_wav_segments(segments: &[Vec<u8>]) -> Result<Vec<u8>> {
    ensure!(!segments.is_empty(), "No WAV segments to concatenate");

    if segments.len() == 1 {
        return Ok(segments[0].clone());
    }

    let first_header = parse_wav_header(&segments[0]).context("Failed to parse first segment")?;

    let mut total_data_size: usize = 0;
    let mut pcm_chunks: Vec<&[u8]> = Vec::with_capacity(segments.len());

    for (i, segment) in segments.iter().enumerate() {
        let header =
            parse_wav_header(segment).with_context(|| format!("Failed to parse segment {i}"))?;
        ensure!(
            header.channels == first_header.channels
                && header.sample_rate == first_header.sample_rate
                && header.bits_per_sample == first_header.bits_per_sample,
            "Segment {i} has incompatible audio format"
        );
        let pcm = &segment[header.data_offset..header.data_offset + header.data_size];
        total_data_size += pcm.len();
        pcm_chunks.push(pcm);
    }

    // Copy everything before the data chunk from the first segment, then write new data chunk
    let pre_data_len = first_header.data_offset - 8; // offset of "data" chunk header
    let output_size = pre_data_len + 8 + total_data_size; // pre-data + data header + PCM
    let file_size = (output_size - 8) as u32; // RIFF size excludes first 8 bytes

    let mut output = Vec::with_capacity(output_size);

    // RIFF header
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&file_size.to_le_bytes());
    output.extend_from_slice(b"WAVE");

    // Chunks before data (fmt, etc.) -- copy from first segment
    output.extend_from_slice(&segments[0][RIFF_HEADER_LEN..pre_data_len]);

    // Data chunk header with combined size
    output.extend_from_slice(b"data");
    output.extend_from_slice(&(total_data_size as u32).to_le_bytes());

    // Combined PCM data
    for pcm in &pcm_chunks {
        output.extend_from_slice(pcm);
    }

    Ok(output)
}

struct WavHeader {
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    data_offset: usize,
    data_size: usize,
}

fn parse_wav_header(data: &[u8]) -> Result<WavHeader> {
    ensure!(data.len() >= RIFF_HEADER_LEN, "WAV data too short");
    ensure!(&data[0..4] == b"RIFF", "Missing RIFF marker");
    ensure!(&data[8..12] == b"WAVE", "Missing WAVE marker");

    // Find fmt chunk
    let mut pos = 12;
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut found_fmt = false;

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;

        if chunk_id == b"fmt " {
            ensure!(chunk_size >= 16, "fmt chunk too small");
            let fmt_data = &data[pos + 8..];
            channels = u16::from_le_bytes([fmt_data[2], fmt_data[3]]);
            sample_rate = u32::from_le_bytes([fmt_data[4], fmt_data[5], fmt_data[6], fmt_data[7]]);
            bits_per_sample = u16::from_le_bytes([fmt_data[14], fmt_data[15]]);
            found_fmt = true;
        }

        if chunk_id == b"data" {
            if !found_fmt {
                bail!("data chunk found before fmt chunk");
            }
            return Ok(WavHeader {
                channels,
                sample_rate,
                bits_per_sample,
                data_offset: pos + 8,
                data_size: chunk_size,
            });
        }

        pos += 8 + chunk_size;
        // Align to even boundary
        if pos % 2 != 0 {
            pos += 1;
        }
    }

    bail!("No data chunk found in WAV")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_wav(pcm: &[u8], channels: u16, sample_rate: u32, bits_per_sample: u16) -> Vec<u8> {
        let data_size = pcm.len() as u32;
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
        let block_align = channels * bits_per_sample / 8;
        let file_size = 36 + data_size;

        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.extend_from_slice(pcm);
        wav
    }

    #[test]
    fn single_segment_returns_clone() {
        let wav = make_wav(&[1, 2, 3, 4], 1, 24000, 16);
        let result = concatenate_wav_segments(&[wav.clone()]).unwrap();
        assert_eq!(result, wav);
    }

    #[test]
    fn two_segments_concatenated() {
        let wav1 = make_wav(&[1, 2], 1, 24000, 16);
        let wav2 = make_wav(&[3, 4], 1, 24000, 16);
        let result = concatenate_wav_segments(&[wav1, wav2]).unwrap();
        let header = parse_wav_header(&result).unwrap();
        assert_eq!(header.data_size, 4);
        assert_eq!(
            &result[header.data_offset..header.data_offset + 4],
            &[1, 2, 3, 4]
        );
    }

    #[test]
    fn incompatible_formats_rejected() {
        let wav1 = make_wav(&[1, 2], 1, 24000, 16);
        let wav2 = make_wav(&[3, 4], 2, 24000, 16);
        assert!(concatenate_wav_segments(&[wav1, wav2]).is_err());
    }

    #[test]
    fn empty_segments_rejected() {
        let result = concatenate_wav_segments(&[]);
        assert!(result.is_err());
    }
}
