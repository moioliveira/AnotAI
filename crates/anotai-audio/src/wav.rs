//! WAV PCM 16 bits: escrita (depuração, `audio.saveRecordings`) e leitura
//! (testes e benchmark).

use std::io::{self, Write};

use crate::{Recording, TARGET_RATE, downmix_into, resample};

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

fn u16_at(b: &[u8], i: usize) -> io::Result<u16> {
    b.get(i..i + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| invalid("WAV truncado"))
}

fn u32_at(b: &[u8], i: usize) -> io::Result<u32> {
    b.get(i..i + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| invalid("WAV truncado"))
}

impl Recording {
    /// Lê um WAV PCM 16 bits (qualquer taxa, mono ou estéreo) e converte
    /// para o formato do STT: mono, 16 kHz.
    pub fn read_wav(bytes: &[u8]) -> io::Result<Self> {
        if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
            return Err(invalid("não é um arquivo WAV"));
        }
        let (mut channels, mut rate, mut bits, mut data) = (0u16, 0u32, 0u16, None);
        let mut pos = 12;
        while pos + 8 <= bytes.len() {
            let id = &bytes[pos..pos + 4];
            let len = u32_at(bytes, pos + 4)? as usize;
            let body = pos + 8;
            match id {
                b"fmt " => {
                    if u16_at(bytes, body)? != 1 {
                        return Err(invalid("só WAV PCM é suportado"));
                    }
                    channels = u16_at(bytes, body + 2)?;
                    rate = u32_at(bytes, body + 4)?;
                    bits = u16_at(bytes, body + 14)?;
                }
                b"data" => {
                    let end = (body + len).min(bytes.len());
                    data = Some(&bytes[body..end]);
                }
                _ => {}
            }
            // Blocos têm tamanho par.
            pos = body + len + (len & 1);
        }
        let data = data.ok_or_else(|| invalid("WAV sem bloco de dados"))?;
        if bits != 16 || channels == 0 || rate == 0 {
            return Err(invalid("só WAV PCM 16 bits é suportado"));
        }
        let pcm: Vec<i16> = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&b| i16::from_le_bytes(b))
            .collect();
        let mut mono = Vec::with_capacity(pcm.len() / usize::from(channels));
        downmix_into(&pcm, usize::from(channels), &mut mono);
        let samples =
            resample(&mono, rate, TARGET_RATE).map_err(|e| io::Error::other(e.to_string()))?;
        Ok(Self {
            samples,
            sample_rate: TARGET_RATE,
        })
    }

    /// Grava como WAV PCM 16 bits mono.
    pub fn write_wav<W: Write>(&self, mut out: W) -> io::Result<()> {
        let data_len = u32::try_from(self.samples.len() * 2)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "gravação grande demais"))?;
        let byte_rate = self.sample_rate * 2;
        out.write_all(b"RIFF")?;
        out.write_all(&(36 + data_len).to_le_bytes())?;
        out.write_all(b"WAVEfmt ")?;
        out.write_all(&16u32.to_le_bytes())?; // tamanho do bloco fmt
        out.write_all(&1u16.to_le_bytes())?; // PCM
        out.write_all(&1u16.to_le_bytes())?; // mono
        out.write_all(&self.sample_rate.to_le_bytes())?;
        out.write_all(&byte_rate.to_le_bytes())?;
        out.write_all(&2u16.to_le_bytes())?; // block align
        out.write_all(&16u16.to_le_bytes())?; // bits por amostra
        out.write_all(b"data")?;
        out.write_all(&data_len.to_le_bytes())?;
        for &s in &self.samples {
            let v = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            out.write_all(&v.to_le_bytes())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_valid_header_and_samples() -> io::Result<()> {
        let rec = Recording {
            samples: vec![0.0, 1.0, -1.0, 2.0],
            sample_rate: 16_000,
        };
        let mut buf = Vec::new();
        rec.write_wav(&mut buf)?;
        assert_eq!(buf.len(), 44 + 8);
        assert_eq!(&buf[0..4], b"RIFF");
        assert_eq!(&buf[8..16], b"WAVEfmt ");
        assert_eq!(
            u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            16_000
        );
        let samples: Vec<i16> = buf[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&b| i16::from_le_bytes(b))
            .collect();
        assert_eq!(samples, [0, i16::MAX, -i16::MAX, i16::MAX]);
        Ok(())
    }

    #[test]
    fn read_round_trips_write() -> io::Result<()> {
        let rec = Recording {
            samples: vec![0.0, 0.5, -0.5, 0.25],
            sample_rate: TARGET_RATE,
        };
        let mut buf = Vec::new();
        rec.write_wav(&mut buf)?;
        let back = Recording::read_wav(&buf)?;
        assert_eq!(back.sample_rate, TARGET_RATE);
        assert_eq!(back.samples.len(), 4);
        for (a, b) in back.samples.iter().zip(&rec.samples) {
            assert!((a - b).abs() < 1e-3);
        }
        Ok(())
    }

    #[test]
    fn read_rejects_non_wav() {
        assert!(Recording::read_wav(b"not a wav file at all").is_err());
    }
}
