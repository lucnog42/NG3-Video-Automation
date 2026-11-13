//! Player dentro do painel. O Rune não decodifica vídeo no Linux, então:
//!
//!   * o FFmpeg decodifica em tempo real (`-re`) para RGBA cru, já reduzido (até 720 px), e cada
//!     quadro vai para um `rune::Pixels` (`set` aceita outra thread e redesenha só a imagem);
//!   * o som sai do mpv (`--no-video`), aberto quando chega o primeiro quadro, para começarem
//!     juntos; o relógio do player conta a partir desse instante.
//!
//! Pausar mata os dois e guarda o tempo; tocar de novo abre a partir dali. Um trecho (o corte)
//! para sozinho no fim.

use rune::Pixels;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const LADO: u32 = 720;

#[derive(Default)]
struct Estado {
    /// quando o primeiro quadro chegou (e o som começou)
    inicio: Option<Instant>,
    /// o FFmpeg terminou (fim do vídeo ou do trecho)
    acabou: bool,
    filhos: Vec<Child>,
}

pub struct Player {
    pub pixels: Pixels,
    /// o vídeo que está tocando (a chave que quem chamou usa: o caminho)
    pub video: Option<String>,
    t0: f32,
    ate: Option<f32>,
    estado: Arc<Mutex<Estado>>,
    geracao: Arc<AtomicU64>,
}

impl Player {
    pub fn novo() -> Player {
        Player { pixels: Pixels::new(), video: None, t0: 0.0, ate: None, estado: Arc::default(), geracao: Arc::default() }
    }

    /// Toca `arquivo` de `ini` até `fim` (ou o fim do vídeo). `w`/`h`: tamanho do vídeo (proporção).
    pub fn tocar(&mut self, arquivo: &Path, chave: &str, ini: f32, fim: Option<f32>, w: u32, h: u32) -> Result<(), String> {
        self.parar();
        // a caixa sai da proporção que quem chamou conhece, mas o FFmpeg encaixa o vídeo nela sem
        // deformar (o que sobra vira faixa preta): se a proporção estiver errada (tira ainda não
        // carregada, vídeo de celular girado pelos metadados), o vídeo não fica achatado
        let (w, h) = (w.max(2), h.max(2));
        let (lw, lh) = if w >= h { (LADO, (LADO * h / w / 2 * 2).max(2)) } else { ((LADO * w / h / 2 * 2).max(2), LADO) };
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-nostdin", "-hide_banner", "-loglevel", "error", "-re", "-ss", &format!("{ini:.3}"), "-i"])
            .arg(arquivo);
        if let Some(f) = fim { cmd.args(["-t", &format!("{:.3}", (f - ini).max(0.1))]); }
        let filtro = format!("scale={lw}:{lh}:force_original_aspect_ratio=decrease,pad={lw}:{lh}:(ow-iw)/2:(oh-ih)/2:black");
        cmd.args(["-an", "-vf", &filtro, "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null());
        let mut ff = cmd.spawn().map_err(|e| format!("não abriu o ffmpeg ({e})"))?;
        let mut saida = ff.stdout.take().ok_or("ffmpeg sem saída")?;
        let geracao_atual = self.geracao.fetch_add(1, Ordering::SeqCst) + 1;
        *self.estado.lock().unwrap() = Estado { filhos: vec![ff], ..Default::default() };
        self.video = Some(chave.to_string());
        self.t0 = ini;
        self.ate = fim;

        let (pixels, estado, geracao) = (self.pixels.clone(), self.estado.clone(), self.geracao.clone());
        let arquivo = arquivo.to_path_buf();
        std::thread::spawn(move || {
            let tam = (lw * lh * 4) as usize;
            let mut primeiro = true;
            loop {
                let mut buf = vec![0u8; tam];
                if saida.read_exact(&mut buf).is_err() || geracao.load(Ordering::SeqCst) != geracao_atual { break; }
                if primeiro && std::env::var_os("NG3_SEM_SOM").is_some() {
                    primeiro = false;                       // testes e snapshots: só a imagem
                    estado.lock().unwrap().inicio = Some(Instant::now());
                }
                if primeiro {
                    primeiro = false;
                    let mut som = Command::new("mpv");
                    som.args(["--no-video", "--really-quiet", "--no-terminal", &format!("--start={ini:.3}")]);
                    if let Some(f) = fim { som.arg(format!("--end={f:.3}")); }
                    let filho = som.arg(&arquivo).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
                    let mut e = estado.lock().unwrap();
                    if let Ok(c) = filho { e.filhos.push(c); }
                    e.inicio = Some(Instant::now());
                }
                pixels.set(lw, lh, buf);
            }
            if geracao.load(Ordering::SeqCst) == geracao_atual { estado.lock().unwrap().acabou = true; }
        });
        Ok(())
    }

    pub fn parar(&mut self) {
        self.geracao.fetch_add(1, Ordering::SeqCst);
        let mut e = self.estado.lock().unwrap();
        for mut c in e.filhos.drain(..) {
            let _ = c.kill();
            let _ = c.wait();
        }
        *e = Estado::default();
        self.video = None;
    }

    /// Tocando `chave` agora (já com imagem)?
    pub fn mostrando(&self, chave: &str) -> bool {
        self.video.as_deref() == Some(chave) && self.estado.lock().unwrap().inicio.is_some()
    }

    pub fn tocando(&self) -> bool { self.video.is_some() }

    /// O tempo no vídeo agora (o relógio corre desde o primeiro quadro).
    pub fn tempo(&self) -> Option<f32> {
        self.video.as_ref()?;
        let e = self.estado.lock().unwrap();
        let t = match e.inicio { Some(i) => self.t0 + i.elapsed().as_secs_f32(), None => self.t0 };
        Some(self.ate.map_or(t, |f| t.min(f)))
    }

    /// O vídeo (ou o trecho) chegou ao fim: devolve o tempo final e para.
    pub fn terminou(&mut self) -> Option<f32> {
        if !self.estado.lock().unwrap().acabou { return None; }
        let t = self.tempo();
        self.parar();
        t
    }
}

impl Drop for Player {
    fn drop(&mut self) { self.parar(); }
}
