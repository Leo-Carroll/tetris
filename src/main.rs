use macroquad::prelude::*;
use std::fs;
use std::io;
use std::path::PathBuf;

const COLS: usize = 10;
const ROWS: usize = 20;
const LOCK_DELAY: f32 = 0.5;
const MAX_LOCK_RESETS: u8 = 15;

const COLORS: [Color; 7] = [
    Color::new(0.20, 0.85, 0.95, 1.0),
    Color::new(0.98, 0.85, 0.25, 1.0),
    Color::new(0.70, 0.40, 0.95, 1.0),
    Color::new(0.35, 0.90, 0.45, 1.0),
    Color::new(0.95, 0.32, 0.38, 1.0),
    Color::new(0.30, 0.50, 0.98, 1.0),
    Color::new(0.98, 0.60, 0.22, 1.0),
];

const BG: Color = Color::new(0.06, 0.07, 0.11, 1.0);
const PANEL: Color = Color::new(0.11, 0.13, 0.19, 1.0);
const TEXT: Color = Color::new(0.90, 0.93, 1.0, 1.0);
const DIM: Color = Color::new(0.55, 0.60, 0.72, 1.0);

type Shape = [[bool; 4]; 4];

fn window_conf() -> Conf {
    Conf {
        window_width: 800,
        window_height: 720,
        window_title: "Tetris".to_owned(),
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

fn rotate_cw(s: &Shape) -> Shape {
    let mut n = [[false; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            n[r][c] = s[3 - c][r];
        }
    }
    n
}

fn rotate_ccw(s: &Shape) -> Shape {
    let mut n = [[false; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            n[r][c] = s[c][3 - r];
        }
    }
    n
}

#[derive(Clone)]
struct Piece {
    kind: usize,
    shape: Shape,
    x: i32,
    y: i32,
}

impl Piece {
    fn new(kind: usize) -> Self {
        Piece { kind, shape: TETROMINOES[kind], x: 3, y: if kind == 0 { -1 } else { 0 } }
    }
    fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..4).flat_map(move |r| {
            (0..4).filter_map(move |c| {
                if self.shape[r][c] { Some((self.x + c as i32, self.y + r as i32)) } else { None }
            })
        })
    }
}

#[derive(Clone, Copy, PartialEq)]
enum GameMode {
    Infinite,
    Zen,
    Sprint40,
}

impl GameMode {
    fn name(self) -> &'static str {
        match self {
            GameMode::Infinite => "INFINITE",
            GameMode::Zen => "ZEN",
            GameMode::Sprint40 => "40 LINES",
        }
    }

    fn description(self) -> &'static str {
        match self {
            GameMode::Infinite => "Survive as the pace steadily climbs.",
            GameMode::Zen => "A relaxed game at a steady pace.",
            GameMode::Sprint40 => "Clear 40 lines as fast as you can.",
        }
    }

    fn cycle(self, direction: i32) -> Self {
        let next = (self as i32 + direction).rem_euclid(3);
        match next {
            0 => GameMode::Infinite,
            1 => GameMode::Zen,
            _ => GameMode::Sprint40,
        }
    }
}

#[derive(Default)]
struct Records {
    infinite_best: u32,
    zen_best: u32,
    sprint40_best: Option<f32>,
}

impl Records {
    fn score_for(&self, mode: GameMode) -> u32 {
        match mode {
            GameMode::Infinite => self.infinite_best,
            GameMode::Zen => self.zen_best,
            GameMode::Sprint40 => 0,
        }
    }
}

fn records_path() -> io::Result<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("LOCALAPPDATA"))
            .map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share")))
    }
    .or_else(|| std::env::current_dir().ok())
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "could not locate a user data directory"))?;
    Ok(base.join("Tetris").join("records.txt"))
}

fn load_records() -> Records {
    let path = match records_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Unable to locate Tetris records file: {error}");
            return Records::default();
        }
    };
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Records::default(),
        Err(error) => {
            eprintln!("Unable to load Tetris records: {error}");
            return Records::default();
        }
    };

    let mut records = Records::default();
    for (index, line) in contents.lines().enumerate() {
        let Some((key, value)) = line.split_once('=') else {
            eprintln!("Ignoring malformed Tetris record on line {}", index + 1);
            continue;
        };
        match key {
            "version" if value == "1" => {}
            "infinite_best" => match value.parse() {
                Ok(score) => records.infinite_best = score,
                Err(_) => eprintln!("Ignoring invalid Infinite record on line {}", index + 1),
            },
            "zen_best" => match value.parse() {
                Ok(score) => records.zen_best = score,
                Err(_) => eprintln!("Ignoring invalid Zen record on line {}", index + 1),
            },
            "sprint40_best" if value == "none" => {}
            "sprint40_best" => match value.parse::<f32>() {
                Ok(time) if time.is_finite() && time > 0.0 => records.sprint40_best = Some(time),
                _ => eprintln!("Ignoring invalid 40 Lines record on line {}", index + 1),
            },
            _ => eprintln!("Ignoring unknown Tetris record on line {}", index + 1),
        }
    }
    records
}

fn save_records(records: &Records) -> io::Result<()> {
    let path = records_path()?;
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Tetris records path has no parent directory")
    })?;
    fs::create_dir_all(directory)?;
    let sprint = records.sprint40_best.map_or_else(|| "none".to_owned(), |time| time.to_string());
    let contents = format!(
        "version=1\ninfinite_best={}\nzen_best={}\nsprint40_best={sprint}\n",
        records.infinite_best, records.zen_best
    );
    fs::write(path, contents)
}

fn update_records(game: &mut Game, records: &mut Records) -> bool {
    if !matches!(game.state, State::Menu | State::GameOver | State::Finished) {
        return false;
    }
    match game.mode {
        GameMode::Infinite if game.score > records.infinite_best => {
            records.infinite_best = game.score;
            game.best = game.score;
            true
        }
        GameMode::Zen if game.score > records.zen_best => {
            records.zen_best = game.score;
            game.best = game.score;
            true
        }
        GameMode::Sprint40
            if game.state == State::Finished
                && records.sprint40_best.map_or(true, |best| game.elapsed < best) =>
        {
            records.sprint40_best = Some(game.elapsed);
            game.best_time = records.sprint40_best;
            true
        }
        _ => false,
    }
}

#[derive(PartialEq)]
enum State {
    Menu,
    Playing,
    Paused,
    GameOver,
    Finished,
}

struct Game {
    board: [[Option<usize>; COLS]; ROWS],
    cur: Piece,
    queue: Vec<usize>,
    hold: Option<usize>,
    can_hold: bool,
    score: u32,
    lines: u32,
    level: u32,
    fall_timer: f32,
    lock_timer: f32,
    lock_resets: u8,
    state: State,
    clear_flash: Vec<(usize, f32)>,
    das: f32,
    best: u32,
    best_time: Option<f32>,
    mode: GameMode,
    elapsed: f32,
    escape_hold: f32,
    particles: Vec<Particle>,
    popups: Vec<Popup>,
    trails: Vec<Trail>,
    shake: f32,
}

// Positions are in board-cell units so effects scale with the window.
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    size: f32,
    color: Color,
}

struct Popup {
    text: String,
    life: f32,
    color: Color,
}

struct Trail {
    x: i32,
    y0: f32,
    y1: f32,
    life: f32,
    color: Color,
}

fn rnd(a: f32, b: f32) -> f32 {
    macroquad::rand::gen_range(a, b)
}

impl Game {
    fn burst(&mut self, x: f32, y: f32, n: usize, speed: f32, color: Color) {
        for _ in 0..n {
            let a = rnd(0.0, std::f32::consts::TAU);
            let s = rnd(0.2, 1.0) * speed;
            let life = rnd(0.35, 0.9);
            self.particles.push(Particle {
                x,
                y,
                vx: a.cos() * s,
                vy: a.sin() * s - speed * 0.2,
                life,
                max_life: life,
                size: rnd(0.08, 0.2),
                color,
            });
        }
    }

    fn add_shake(&mut self, amount: f32) {
        self.shake = (self.shake + amount).min(1.0);
    }

    fn tick_fx(&mut self, dt: f32) {
        self.shake = (self.shake - dt * 2.2).max(0.0);
        for p in self.particles.iter_mut() {
            p.vy += 30.0 * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        for t in self.trails.iter_mut() {
            t.life -= dt;
        }
        self.trails.retain(|t| t.life > 0.0);
        for p in self.popups.iter_mut() {
            p.life -= dt;
        }
        self.popups.retain(|p| p.life > 0.0);
    }

    fn new(best: u32, best_time: Option<f32>, mode: GameMode) -> Self {
        let mut g = Game {
            board: [[None; COLS]; ROWS],
            cur: Piece::new(0),
            queue: vec![],
            hold: None,
            can_hold: true,
            score: 0,
            lines: 0,
            level: 1,
            fall_timer: 0.0,
            lock_timer: 0.0,
            lock_resets: 0,
            state: State::Playing,
            clear_flash: vec![],
            das: 0.0,
            best,
            particles: vec![],
            popups: vec![],
            trails: vec![],
            shake: 0.0,
            mode,
            elapsed: 0.0,
            escape_hold: 0.0,
            best_time,
        };
        g.cur = Piece::new(g.pop_next());
        g
    }

    fn refill(&mut self) {
        while self.queue.len() < 8 {
            let mut bag: Vec<usize> = (0..7).collect();
            for i in (1..7).rev() {
                let j = macroquad::rand::gen_range(0, i + 1);
                bag.swap(i, j);
            }
            // queue is consumed from the front
            self.queue.extend(bag);
        }
    }

    fn pop_next(&mut self) -> usize {
        self.refill();
        self.queue.remove(0)
    }

    fn valid(&self, p: &Piece) -> bool {
        p.cells().all(|(x, y)| {
            x >= 0 && x < COLS as i32 && y < ROWS as i32 && (y < 0 || self.board[y as usize][x as usize].is_none())
        })
    }

    fn is_grounded(&self) -> bool {
        let mut below = self.cur.clone();
        below.y += 1;
        !self.valid(&below)
    }

    fn reset_lock_delay(&mut self, was_grounded: bool) {
        if was_grounded && self.lock_resets < MAX_LOCK_RESETS {
            self.lock_timer = 0.0;
            self.lock_resets += 1;
        }
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        let mut p = self.cur.clone();
        p.x += dx;
        p.y += dy;
        if self.valid(&p) {
            self.cur = p;
            true
        } else {
            false
        }
    }

    fn try_move_sideways(&mut self, dx: i32) -> bool {
        let was_grounded = self.is_grounded();
        let moved = self.try_move(dx, 0);
        if moved {
            self.reset_lock_delay(was_grounded);
        }
        moved
    }

    fn rotate(&mut self, cw: bool) {
        if self.cur.kind == 1 {
            return;
        }
        let was_grounded = self.is_grounded();
        let mut p = self.cur.clone();
        p.shape = if cw { rotate_cw(&p.shape) } else { rotate_ccw(&p.shape) };
        for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (-2, 0), (2, 0), (-1, -1), (1, -1)] {
            let mut q = p.clone();
            q.x += dx;
            q.y += dy;
            if self.valid(&q) {
                self.cur = q;
                self.reset_lock_delay(was_grounded);
                return;
            }
        }
    }

    fn ghost_y(&self) -> i32 {
        let mut p = self.cur.clone();
        while {
            p.y += 1;
            self.valid(&p)
        } {}
        p.y - 1
    }

    fn hold_piece(&mut self) {
        if !self.can_hold {
            return;
        }
        let k = self.cur.kind;
        let next = match self.hold.replace(k) {
            Some(h) => h,
            None => self.pop_next(),
        };
        self.cur = Piece::new(next);
        self.can_hold = false;
        self.fall_timer = 0.0;
        self.lock_timer = 0.0;
        self.lock_resets = 0;
        if !self.valid(&self.cur) {
            self.state = State::GameOver;
        }
    }

    fn lock(&mut self) {
        let kind = self.cur.kind;
        let mut over = false;
        for (x, y) in self.cur.cells().collect::<Vec<_>>() {
            if y < 0 {
                over = true;
            } else {
                self.board[y as usize][x as usize] = Some(kind);
            }
        }
        let full: Vec<usize> = (0..ROWS).filter(|&r| self.board[r].iter().all(|c| c.is_some())).collect();
        let landed: Vec<(i32, i32)> = self.cur.cells().collect();
        for (x, y) in landed {
            self.burst(x as f32 + 0.5, y as f32 + 1.0, 2, 3.0, COLORS[kind]);
        }
        self.add_shake(0.03);
        if !full.is_empty() {
            for &r in &full {
                for c in 0..COLS {
                    if let Some(k) = self.board[r][c] {
                        self.burst(c as f32 + 0.5, r as f32 + 0.5, 3, 9.0, COLORS[k]);
                    }
                }
            }
            let (label, col) = match full.len() {
                1 => ("SINGLE", TEXT),
                2 => ("DOUBLE", COLORS[0]),
                3 => ("TRIPLE", COLORS[3]),
                _ => ("TETRIS!", COLORS[1]),
            };
            self.popups.push(Popup { text: label.to_string(), life: 1.0, color: col });
            self.add_shake(0.08 + 0.08 * full.len() as f32);
            for &r in &full {
                self.board[..=r].rotate_right(1);
                self.board[0] = [None; COLS];
            }
            let pts = [0, 100, 300, 500, 800][full.len()] * self.level;
            self.score += pts;
            self.lines += full.len() as u32;
            self.level = self.lines / 10 + 1;
            self.clear_flash = full.iter().map(|&r| (r, 0.25)).collect();
        }
        self.can_hold = true;
        self.lock_timer = 0.0;
        self.lock_resets = 0;
        self.fall_timer = 0.0;
        let n = self.pop_next();
        self.cur = Piece::new(n);
        if self.mode == GameMode::Sprint40 && self.lines >= 40 {
            self.state = State::Finished;
        } else if over || !self.valid(&self.cur) {
            self.state = State::GameOver;
        }
        self.best = self.best.max(self.score);
    }

    fn hard_drop(&mut self) {
        let gy = self.ghost_y();
        let dist = (gy - self.cur.y).max(0);
        self.score += 2 * dist as u32;
        let col = COLORS[self.cur.kind];
        let trails: Vec<Trail> = self
            .cur
            .cells()
            .map(|(x, y)| Trail { x, y0: y as f32, y1: (y + dist) as f32, life: 0.25, color: col })
            .collect();
        self.trails.extend(trails);
        self.add_shake((0.05 + dist as f32 * 0.008).min(0.2));
        self.cur.y = gy;
        self.lock();
    }

    fn interval(&self) -> f32 {
        if self.mode == GameMode::Zen {
            return 0.8;
        }
        let level_speedup = (self.level as f32 - 1.0) * 0.07;
        let score_speedup = (self.score / 1_000) as f32 * 0.03;
        (0.8 - level_speedup - score_speedup).max(0.05)
    }

    fn update(&mut self, dt: f32) {
        self.elapsed += dt;
        for f in self.clear_flash.iter_mut() {
            f.1 -= dt;
        }
        self.clear_flash.retain(|f| f.1 > 0.0);

        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            self.try_move_sideways(-1);
            self.das = -0.17;
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            self.try_move_sideways(1);
            self.das = -0.17;
        }
        let dir = (is_key_down(KeyCode::Right) || is_key_down(KeyCode::D)) as i32
            - (is_key_down(KeyCode::Left) || is_key_down(KeyCode::A)) as i32;
        if dir != 0 {
            self.das += dt;
            while self.das >= 0.05 {
                self.das -= 0.05;
                self.try_move_sideways(dir);
            }
        }
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::X) || is_key_pressed(KeyCode::W) {
            self.rotate(true);
        }
        if is_key_pressed(KeyCode::Z) {
            self.rotate(false);
        }
        if is_key_pressed(KeyCode::C) || is_key_pressed(KeyCode::LeftShift) {
            self.hold_piece();
            if self.state == State::GameOver {
                return;
            }
        }
        if is_key_pressed(KeyCode::Space) {
            self.hard_drop();
            return;
        }

        let soft = is_key_down(KeyCode::Down) || is_key_down(KeyCode::S);
        let interval = if soft { self.interval().min(0.04) } else { self.interval() };
        self.fall_timer = self.fall_timer.min(interval);
        self.fall_timer += dt;
        while self.fall_timer >= interval {
            self.fall_timer -= interval;
            if self.try_move(0, 1) {
                if soft {
                    self.score += 1;
                }
            } else {
                break;
            }
        }
        let mut grounded = self.cur.clone();
        grounded.y += 1;
        if !self.valid(&grounded) {
            self.lock_timer += dt;
            if self.lock_timer >= LOCK_DELAY {
                self.lock();
            }
        } else {
            self.lock_timer = 0.0;
        }
    }
}

fn shade(c: Color, k: f32) -> Color {
    Color::new((c.r * k).min(1.0), (c.g * k).min(1.0), (c.b * k).min(1.0), c.a)
}

fn draw_block(x: f32, y: f32, s: f32, c: Color) {
    let g = (s * 0.04).max(1.0);
    draw_rectangle(x, y, s, s, shade(c, 0.55));
    draw_rectangle(x + g, y + g, s - 2.0 * g, s - 2.0 * g, c);
    draw_rectangle(x + g, y + g, s - 2.0 * g, s * 0.22, shade(c, 1.25));
    draw_rectangle(x + g, y + s - g - s * 0.12, s - 2.0 * g, s * 0.12, shade(c, 0.8));
}

fn make_glow_texture() -> Texture2D {
    const SIZE: u16 = 64;
    let mut image = Image::gen_image_color(SIZE, SIZE, WHITE);
    let center = (SIZE as f32 - 1.0) * 0.5;
    let radius = center;
    for y in 0..SIZE as u32 {
        for x in 0..SIZE as u32 {
            let dx = (x as f32 - center) / radius;
            let dy = (y as f32 - center) / radius;
            let falloff = (1.0 - (dx * dx + dy * dy).sqrt()).max(0.0);
            image.set_pixel(x, y, Color::new(1.0, 1.0, 1.0, falloff * falloff * 0.22));
        }
    }
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Linear);
    texture
}

fn glow(texture: &Texture2D, x: f32, y: f32, w: f32, h: f32, reach: f32, c: Color, strength: f32) {
    let size = vec2(w + 2.0 * reach, h + 2.0 * reach);
    draw_texture_ex(
        texture,
        x - reach,
        y - reach,
        Color::new(c.r, c.g, c.b, strength),
        DrawTextureParams {
            dest_size: Some(size),
            ..Default::default()
        },
    );
}

fn text_c(t: &str, cx: f32, y: f32, size: f32, col: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    draw_text(t, cx - d.width / 2.0, y, size, col);
}

fn format_time(seconds: f32) -> String {
    let centiseconds = (seconds.max(0.0) * 100.0) as u32;
    format!(
        "{:02}:{:02}.{:02}",
        centiseconds / 6_000,
        centiseconds / 100 % 60,
        centiseconds % 100
    )
}

fn panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, PANEL);
    draw_rectangle_lines(x, y, w, h, 2.0, Color::new(1.0, 1.0, 1.0, 0.06));
}

fn draw_mini(kind: usize, x: f32, y: f32, w: f32, h: f32, cell: f32) {
    let s = &TETROMINOES[kind];
    let (mut minc, mut maxc, mut minr, mut maxr) = (4, 0, 4, 0);
    for r in 0..4 {
        for c in 0..4 {
            if s[r][c] {
                minc = minc.min(c);
                maxc = maxc.max(c);
                minr = minr.min(r);
                maxr = maxr.max(r);
            }
        }
    }
    let pw = (maxc - minc + 1) as f32 * cell;
    let ph = (maxr - minr + 1) as f32 * cell;
    let ox = x + (w - pw) / 2.0;
    let oy = y + (h - ph) / 2.0;
    for r in minr..=maxr {
        for c in minc..=maxc {
            if s[r][c] {
                draw_block(ox + (c - minc) as f32 * cell, oy + (r - minr) as f32 * cell, cell, COLORS[kind]);
            }
        }
    }
}

fn draw_game(g: &Game, glow_texture: &Texture2D) {
    let sw = screen_width();
    let sh = screen_height();
    clear_background(BG);

    let narrow = sw < sh * 0.9;
    // Cell size: fit the board plus side panels (or panels below board when narrow).
    let cell = if narrow {
        ((sh * 0.72) / ROWS as f32).min(sw * 0.94 / COLS as f32)
    } else {
        ((sh * 0.92) / ROWS as f32).min(sw * 0.96 / (COLS as f32 + 9.0))
    };
    let bw = cell * COLS as f32;
    let bh = cell * ROWS as f32;
    let side_w = if narrow { bw } else { cell * 4.5 };
    let gap = cell * 0.5;
    let total_w = if narrow { bw } else { bw + 2.0 * (side_w + gap) };
    let ox = (sw - total_w) / 2.0;
    let (bx, by) = if narrow {
        (ox, sh * 0.02)
    } else {
        (ox + side_w + gap, (sh - bh) / 2.0)
    };

    {
    let amp = g.shake * g.shake * cell * 0.18;
    let (bx, by) = (bx + rnd(-1.0, 1.0) * amp, by + rnd(-1.0, 1.0) * amp);
    // Board
    glow(glow_texture, bx - 4.0, by - 4.0, bw + 8.0, bh + 8.0, cell * 0.18, Color::new(0.3, 0.5, 1.0, 1.0), 0.25);
    draw_rectangle(bx - 4.0, by - 4.0, bw + 8.0, bh + 8.0, Color::new(0.2, 0.3, 0.5, 0.5));
    draw_rectangle(bx, by, bw, bh, Color::new(0.04, 0.05, 0.08, 1.0));
    for i in 1..COLS {
        draw_line(bx + i as f32 * cell, by, bx + i as f32 * cell, by + bh, 1.0, Color::new(1.0, 1.0, 1.0, 0.04));
    }
    for i in 1..ROWS {
        draw_line(bx, by + i as f32 * cell, bx + bw, by + i as f32 * cell, 1.0, Color::new(1.0, 1.0, 1.0, 0.04));
    }
    for r in 0..ROWS {
        for c in 0..COLS {
            if let Some(k) = g.board[r][c] {
                let (px, py) = (bx + c as f32 * cell, by + r as f32 * cell);
                draw_block(px, py, cell, COLORS[k]);
            }
        }
    }
    for &(r, t) in &g.clear_flash {
        draw_rectangle(bx, by + r as f32 * cell, bw, cell, Color::new(1.0, 1.0, 1.0, (t / 0.25).min(1.0) * 0.8));
    }
    if g.state != State::GameOver {
        let gy = g.ghost_y();
        let col = COLORS[g.cur.kind];
        for (x, y) in g.cur.cells() {
            let y = y + gy - g.cur.y;
            if y >= 0 {
                let (px, py) = (bx + x as f32 * cell, by + y as f32 * cell);
                draw_rectangle(px, py, cell, cell, Color::new(col.r, col.g, col.b, 0.12));
                draw_rectangle_lines(px + 1.0, py + 1.0, cell - 2.0, cell - 2.0, 2.0, Color::new(col.r, col.g, col.b, 0.5));
            }
        }
        for (x, y) in g.cur.cells() {
            if y >= 0 {
                let (px, py) = (bx + x as f32 * cell, by + y as f32 * cell);
                glow(glow_texture, px, py, cell, cell, cell * 0.3, col, 0.65);
                draw_block(px, py, cell, col);
            }
        }
        if g.is_grounded() {
            let mut cells = g.cur.cells().filter(|(_, y)| *y >= 0);
            if let Some((first_x, first_y)) = cells.next() {
                let (min_x, max_x, max_y) = cells.fold(
                    (first_x, first_x, first_y),
                    |(min_x, max_x, max_y), (x, y)| (min_x.min(x), max_x.max(x), max_y.max(y)),
                );
                let bar_x = bx + min_x as f32 * cell;
                let bar_y = by + (max_y + 1) as f32 * cell - 4.0;
                let bar_w = (max_x - min_x + 1) as f32 * cell;
                draw_rectangle(bar_x, bar_y, bar_w, 4.0, Color::new(0.15, 0.17, 0.22, 0.95));
                draw_rectangle(
                    bar_x,
                    bar_y,
                    bar_w * (g.lock_timer / LOCK_DELAY).clamp(0.0, 1.0),
                    4.0,
                    Color::new(1.0, 0.35, 0.25, 1.0),
                );
            }
        }
    }

    for t in &g.trails {
        let a = t.life / 0.25;
        let top = by + t.y0.max(0.0) * cell;
        let bot = by + t.y1 * cell;
        draw_rectangle(
            bx + t.x as f32 * cell + cell * 0.15,
            top,
            cell * 0.7,
            (bot - top).max(0.0),
            Color::new(t.color.r, t.color.g, t.color.b, a * 0.3),
        );
    }
    for p in &g.particles {
        let a = (p.life / p.max_life).clamp(0.0, 1.0);
        let s = p.size * cell * (0.4 + 0.6 * a);
        let (px, py) = (bx + p.x * cell - s / 2.0, by + p.y * cell - s / 2.0);
        glow(glow_texture, px, py, s, s, s * 0.8, p.color, a * 0.55);
        draw_rectangle(px, py, s, s, Color::new(1.0, 1.0, 1.0, a * 0.6));
    }
    for p in &g.popups {
        let k = 1.0 - p.life;
        let size = cell * (1.0 + 0.5 * (1.0 - k * 4.0).max(0.0));
        let gc = Color::new(p.color.r, p.color.g, p.color.b, (p.life * 2.0).min(1.0) * 0.3);
        for (dx, dy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)] {
            text_c(&p.text, bx + bw / 2.0 + dx, by + bh * 0.4 - k * cell * 2.0 + dy, size, gc);
        }
        text_c(
            &p.text,
            bx + bw / 2.0,
            by + bh * 0.4 - k * cell * 2.0,
            size,
            Color::new(p.color.r, p.color.g, p.color.b, (p.life * 2.0).min(1.0)),
        );
    }
    }

    // Side panels
    let title = (cell * 0.5).max(12.0);
    let val = (cell * 0.9).max(16.0);
    let box_h = cell * 3.2;
    let (lx, ly, rx, ry, pw) = if narrow {
        let y = by + bh + 12.0;
        (bx, y, bx, y, bw)
    } else {
        (bx - gap - side_w, by, bx + bw + gap, by, side_w)
    };

    let stat = |x: f32, y: f32, w: f32, label: &str, value: String| {
        panel(x, y, w, cell * 1.9);
        text_c(label, x + w / 2.0, y + title * 1.4, title, DIM);
        text_c(&value, x + w / 2.0, y + cell * 1.6, val, TEXT);
    };

    if narrow {
        let w3 = (pw - 2.0 * 8.0) / 3.0;
        stat(lx, ly, w3, "SCORE", g.score.to_string());
        if g.mode == GameMode::Sprint40 {
            stat(lx + w3 + 8.0, ly, w3, "TIME", format_time(g.elapsed));
            stat(lx + 2.0 * (w3 + 8.0), ly, w3, "LINES", format!("{}/40", g.lines.min(40)));
        } else {
            stat(lx + w3 + 8.0, ly, w3, "LEVEL", g.level.to_string());
            stat(lx + 2.0 * (w3 + 8.0), ly, w3, "LINES", g.lines.to_string());
        }
        let y2 = ly + cell * 1.9 + 8.0;
        let bh2 = (sh - y2 - 8.0).max(cell * 2.0);
        let w2 = (pw - 8.0) / 2.0;
        panel(lx, y2, w2, bh2);
        text_c("HOLD", lx + w2 / 2.0, y2 + title * 1.4, title, DIM);
        panel(lx + w2 + 8.0, y2, w2, bh2);
        text_c("NEXT", lx + w2 * 1.5 + 8.0, y2 + title * 1.4, title, DIM);
        let mc = (bh2 * 0.22).min(cell * 0.8);
        if let Some(h) = g.hold {
            draw_mini(h, lx, y2 + title * 1.6, w2, bh2 - title * 1.6, mc);
        }
        draw_mini(g.queue[0], lx + w2 + 8.0, y2 + title * 1.6, w2, bh2 - title * 1.6, mc);
    } else {
        // Left: hold + controls
        panel(lx, ly, pw, box_h);
        text_c("HOLD", lx + pw / 2.0, ly + title * 1.6, title, DIM);
        if let Some(h) = g.hold {
            let col = if g.can_hold { 1.0 } else { 0.4 };
            let _ = col;
            draw_mini(h, lx, ly + title * 2.0, pw, box_h - title * 2.0, cell * 0.7);
        }
        let cy = ly + box_h + gap;
        let ch = bh - box_h - gap;
        panel(lx, cy, pw, ch);
        let small = (cell * 0.42).max(10.0);
        text_c("CONTROLS", lx + pw / 2.0, cy + title * 1.6, title, DIM);
        text_c(g.mode.name(), lx + pw / 2.0, cy + title * 2.65, small, COLORS[0]);
        let lines = [
            "←/→  Move",
            "↑/X  Rotate",
            "Z  Rotate back",
            "↓  Soft drop",
            "Space  Hard drop",
            "C  Hold",
            "P  Pause",
            "Hold Esc  Menu",
            "Ground bar  Lock timer",
        ];
        for (i, l) in lines.iter().enumerate() {
            draw_text(&l.replace('←', "<").replace('→', ">").replace('↑', "^").replace('↓', "v"), lx + cell * 0.4, cy + title * 3.2 + i as f32 * small * 1.6, small, TEXT);
        }

        // Right: next + stats
        panel(rx, ry, pw, box_h * 1.9);
        text_c("NEXT", rx + pw / 2.0, ry + title * 1.6, title, DIM);
        for i in 0..3 {
            let top = ry + title * 2.0 + i as f32 * (box_h * 1.9 - title * 2.0) / 3.0;
            draw_mini(g.queue[i], rx, top, pw, (box_h * 1.9 - title * 2.0) / 3.0, cell * if i == 0 { 0.7 } else { 0.55 });
        }
        let sy = ry + box_h * 1.9 + gap;
        let sh2 = cell * 1.9 + gap;
        stat(rx, sy, pw, "SCORE", g.score.to_string());
        if g.mode == GameMode::Sprint40 {
            stat(rx, sy + sh2, pw, "TIME", format_time(g.elapsed));
            stat(
                rx,
                sy + sh2 * 2.0,
                pw,
                "BEST",
                g.best_time.map_or_else(|| "--:--.--".to_owned(), format_time),
            );
        } else {
            stat(rx, sy + sh2, pw, "BEST", g.best.to_string());
            stat(rx, sy + sh2 * 2.0, pw, "LEVEL", g.level.to_string());
        }
        let line_label = if g.mode == GameMode::Sprint40 { "PROGRESS" } else { "LINES" };
        let line_value = if g.mode == GameMode::Sprint40 {
            format!("{}/40", g.lines.min(40))
        } else {
            g.lines.to_string()
        };
        stat(rx, sy + sh2 * 3.0, pw, line_label, line_value);
    }

    // Overlays
    if g.state != State::Playing {
        draw_rectangle(bx, by, bw, bh, Color::new(0.0, 0.0, 0.0, 0.7));
        let (t, sub) = match g.state {
            State::Paused => ("PAUSED", "Press P to resume"),
            State::Finished => ("40 LINES CLEARED!", "Press Enter to race again  ·  M for menu"),
            _ => ("GAME OVER", "Press Enter to restart  ·  M for menu"),
        };
        text_c(t, bx + bw / 2.0, by + bh / 2.0 - cell * 0.3, cell * 1.4, TEXT);
        text_c(sub, bx + bw / 2.0, by + bh / 2.0 + cell * 0.8, cell * 0.6, DIM);
        if g.escape_hold > 0.0 {
            text_c("KEEP HOLDING ESC TO RETURN", bx + bw / 2.0, by + bh / 2.0 + cell * 1.5, cell * 0.42, COLORS[0]);
            draw_rectangle(
                bx + bw * 0.2,
                by + bh / 2.0 + cell * 1.75,
                bw * 0.6 * (g.escape_hold / 0.75).min(1.0),
                cell * 0.08,
                COLORS[0],
            );
        }
    }
}

fn draw_menu(glow_texture: &Texture2D, records: &Records, selected_mode: GameMode) {
    let sw = screen_width();
    let sh = screen_height();
    clear_background(BG);

    let ui_scale = (sh / 720.0).clamp(0.65, 1.0);
    let title_size = (sw.min(sh) * 0.12).clamp(38.0, 88.0);
    let center_x = sw * 0.5;
    let center_y = sh * 0.27;
    let title = "TETRIS";
    let title_width = measure_text(title, None, title_size as u16, 1.0).width;
    glow(
        glow_texture,
        center_x - title_width * 0.5,
        center_y - title_size,
        title_width,
        title_size,
        title_size * 0.55,
        COLORS[0],
        0.5,
    );
    text_c(title, center_x, center_y, title_size, TEXT);
    text_c("A classic, with a little extra polish", center_x, center_y + title_size * 0.65, title_size * 0.24, DIM);

    let row_w = (sw * 0.88).min(600.0);
    let gap = 10.0 * ui_scale;
    let card_w = (row_w - gap * 2.0) / 3.0;
    let card_h = 108.0 * ui_scale;
    let row_x = center_x - row_w * 0.5;
    let row_y = center_y + title_size * 1.0;
    let modes = [GameMode::Infinite, GameMode::Zen, GameMode::Sprint40];
    for (i, mode) in modes.into_iter().enumerate() {
        let x = row_x + i as f32 * (card_w + gap);
        let selected = mode == selected_mode;
        draw_rectangle(
            x,
            row_y,
            card_w,
            card_h,
            if selected { Color::new(0.16, 0.22, 0.32, 1.0) } else { PANEL },
        );
        draw_rectangle_lines(
            x,
            row_y,
            card_w,
            card_h,
            if selected { 2.0 } else { 1.0 },
            if selected { COLORS[0] } else { Color::new(1.0, 1.0, 1.0, 0.06) },
        );
        text_c(mode.name(), x + card_w * 0.5, row_y + 39.0 * ui_scale, (card_w * 0.15).clamp(12.0, 18.0), if selected { TEXT } else { DIM });
        text_c(
            match mode {
                GameMode::Infinite => "ENDLESS",
                GameMode::Zen => "RELAXED",
                GameMode::Sprint40 => "TIMED",
            },
            x + card_w * 0.5,
            row_y + 72.0 * ui_scale,
            (card_w * 0.1).clamp(10.0, 13.0),
            DIM,
        );
    }

    let pulse = 0.75 + 0.25 * (get_time() as f32 * 3.0).sin().abs();
    text_c(
        selected_mode.description(),
        center_x,
        row_y + card_h + 31.0 * ui_scale,
        ((sw * 0.035).clamp(14.0, 19.0) * ui_scale).max(11.0),
        TEXT,
    );
    text_c(
        "← / →  Choose mode       ENTER / SPACE  Start",
        center_x,
        row_y + card_h + 69.0 * ui_scale,
        ((sw * 0.03).clamp(12.0, 16.0) * ui_scale).max(10.0),
        Color::new(0.55, 0.88, 1.0, pulse),
    );
    text_c(
        "Move: ← → / A D     Rotate: ↑ / X     Soft drop: ↓ / S     Hold: C",
        center_x,
        row_y + card_h + 101.0 * ui_scale,
        ((sw * 0.025).clamp(10.0, 14.0) * ui_scale).max(9.0),
        DIM,
    );

    let record_text = match selected_mode {
        GameMode::Infinite => format!("INFINITE BEST SCORE  {}", records.infinite_best),
        GameMode::Zen => format!("ZEN BEST SCORE  {}", records.zen_best),
        GameMode::Sprint40 => records
            .sprint40_best
            .map_or_else(|| "40 LINES BEST TIME  --:--.--".to_owned(), |time| format!("40 LINES BEST TIME  {}", format_time(time))),
    };
    text_c(
        &record_text,
        center_x,
        row_y + card_h + 134.0 * ui_scale,
        16.0 * ui_scale,
        DIM,
    );
}

#[macroquad::main(window_conf)]
async fn main() {
    macroquad::rand::srand((get_time() * 1_000_000.0) as u64);
    let glow_texture = make_glow_texture();
    let mut records = load_records();
    let mut game = Game::new(records.score_for(GameMode::Infinite), records.sprint40_best, GameMode::Infinite);
    game.state = State::Menu;
    loop {
        let dt = get_frame_time().min(0.1);
        if game.state != State::Menu && is_key_down(KeyCode::Escape) {
            game.escape_hold += dt;
            if game.escape_hold >= 0.75 {
                game.state = State::Menu;
                game.escape_hold = 0.0;
            }
        } else if !is_key_down(KeyCode::Escape) {
            game.escape_hold = 0.0;
        }
        if game.state != State::Paused {
            game.tick_fx(dt);
        }
        match game.state {
            State::Menu => {
                if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                    game.mode = game.mode.cycle(-1);
                } else if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                    game.mode = game.mode.cycle(1);
                }
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    game = Game::new(records.score_for(game.mode), records.sprint40_best, game.mode);
                }
            }
            State::Playing => {
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    game.state = State::Paused;
                } else {
                    game.update(dt);
                }
            }
            State::Paused => {
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    game.state = State::Playing;
                }
            }
            State::GameOver => {
                if is_key_pressed(KeyCode::Enter) {
                    game = Game::new(records.score_for(game.mode), records.sprint40_best, game.mode);
                } else if is_key_pressed(KeyCode::M) {
                    game.state = State::Menu;
                }
            }
            State::Finished => {
                if is_key_pressed(KeyCode::Enter) {
                    game = Game::new(records.score_for(game.mode), records.sprint40_best, game.mode);
                } else if is_key_pressed(KeyCode::M) {
                    game.state = State::Menu;
                }
            }
        }
        if update_records(&mut game, &mut records) {
            if let Err(error) = save_records(&records) {
                eprintln!("Unable to save Tetris records: {error}");
            }
        }
        if game.state == State::Menu {
            draw_menu(&glow_texture, &records, game.mode);
        } else {
            draw_game(&game, &glow_texture);
        }
        next_frame().await;
    }
}

pub const TETROMINOES: [[[bool; 4]; 4]; 7] = [
    [
        [false, false, false, false],
        [true,  true,  true,  true ],
        [false, false, false, false],
        [false, false, false, false],
    ],
    [
        [false, true,  true,  false],
        [false, true,  true,  false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    [
        [false, true,  false, false],
        [true,  true,  true,  false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    [
        [false, true,  true,  false],
        [true,  true,  false, false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    [
        [true,  true,  false, false],
        [false, true,  true,  false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    [
        [true,  false, false, false],
        [true,  true,  true,  false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    [
        [false, false, true,  false],
        [true,  true,  true,  false],
        [false, false, false, false],
        [false, false, false, false],
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    fn new_game() -> Game {
        Game::new(0, None, GameMode::Infinite)
    }

    fn ground_active_piece(game: &mut Game) {
        game.board[ROWS - 1] = [Some(0); COLS];
        while game.try_move(0, 1) {}
        assert!(game.is_grounded());
    }

    #[test]
    fn holding_piece_with_blocked_spawn_ends_game() {
        let mut game = new_game();
        game.hold = Some(0);
        game.board[0][3] = Some(1);

        game.hold_piece();

        assert!(game.state == State::GameOver);
        assert_eq!(game.cur.kind, 0);
        assert!(!game.can_hold);
    }

    #[test]
    fn grounded_sideways_move_resets_lock_delay() {
        let mut game = new_game();
        ground_active_piece(&mut game);
        game.lock_timer = LOCK_DELAY * 0.8;

        assert!(game.try_move_sideways(1));

        assert_eq!(game.lock_timer, 0.0);
        assert_eq!(game.lock_resets, 1);
    }

    #[test]
    fn grounded_sideways_move_respects_lock_reset_cap() {
        let mut game = new_game();
        ground_active_piece(&mut game);
        game.lock_timer = LOCK_DELAY * 0.8;
        game.lock_resets = MAX_LOCK_RESETS;

        assert!(game.try_move_sideways(1));

        assert_eq!(game.lock_timer, LOCK_DELAY * 0.8);
        assert_eq!(game.lock_resets, MAX_LOCK_RESETS);
    }
}