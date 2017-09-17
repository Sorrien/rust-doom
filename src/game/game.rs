use super::SHADER_ROOT;
use super::ctrl::{GameController, Gesture};
use super::errors::{Result, ErrorKind};
use super::level::Level;
use super::player::Player;
use gfx::{Scene, SceneBuilder, Window};
use gfx::TextRenderer;
use math::Vec2f;
use sdl2::{self, Sdl};
use sdl2::keyboard::Scancode;
use std::path::PathBuf;
use time;
use wad::{Archive, TextureDirectory};

pub struct GameConfig {
    pub wad_file: PathBuf,
    pub metadata_file: PathBuf,
    pub level_index: usize,
    pub fov: f32,
    pub width: u32,
    pub height: u32,
}


pub struct Game {
    window: Window,
    scene: Scene,
    text: TextRenderer,
    player: Player,
    level: Level,
    _sdl: Sdl,
    control: GameController,
}

impl Game {
    pub fn new(config: GameConfig) -> Result<Game> {
        let sdl = sdl2::init().map_err(ErrorKind::Sdl)?;
        let window = Window::new(&sdl, config.width, config.height)?;
        let wad = Archive::open(&config.wad_file, &config.metadata_file)?;
        ensure!(
            config.level_index < wad.num_levels(),
            "Level index was {}, must be between 0..{}, run with --list-levels to see names.",
            config.level_index,
            wad.num_levels() - 1
        );
        let textures = TextureDirectory::from_archive(&wad)?;
        let (level, scene) = {
            let mut scene = SceneBuilder::new(&window, PathBuf::from(SHADER_ROOT));
            let level = Level::new(&wad, &textures, config.level_index, &mut scene)?;
            let scene = scene.build()?;
            (level, scene)
        };

        let mut player = Player::new(config.fov, window.aspect_ratio() * 1.2, Default::default());
        player.set_position(level.start_pos());

        let control = GameController::new(&sdl, sdl.event_pump().map_err(ErrorKind::Sdl)?);

        let text = TextRenderer::new(&window)?;

        Ok(Game {
            window: window,
            player: player,
            level: level,
            scene: scene,
            text: text,
            _sdl: sdl,
            control: control,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let quit_gesture = Gesture::AnyOf(vec![
            Gesture::QuitTrigger,
            Gesture::KeyTrigger(Scancode::Escape),
        ]);
        let grab_toggle_gesture = Gesture::KeyTrigger(Scancode::Grave);
        let help_gesture = Gesture::KeyTrigger(Scancode::H);

        let short_help = self.text.insert(
            &self.window,
            SHORT_HELP,
            Vec2f::new(0.0, 0.0),
            6,
        );
        let long_help = self.text.insert(
            &self.window,
            LONG_HELP,
            Vec2f::new(0.0, 0.0),
            6,
        );
        self.text[long_help].set_visible(false);
        let mut current_help = 0;

        let mut cum_time = 0.0;
        let mut cum_updates_time = 0.0;
        let mut num_frames = 0.0;
        let mut t0 = time::precise_time_s();
        let mut mouse_grabbed = true;
        let mut running = true;
        while running {
            let mut frame = self.window.draw();
            let t1 = time::precise_time_s();
            let mut delta = (t1 - t0) as f32;
            if delta < 1e-10 {
                delta = 1.0 / 60.0;
            }
            let delta = delta;
            t0 = t1;

            let updates_t0 = time::precise_time_s();

            self.control.update();
            if self.control.poll_gesture(&quit_gesture) {
                running = false;
            } else if self.control.poll_gesture(&grab_toggle_gesture) {
                mouse_grabbed = !mouse_grabbed;
                self.control.set_mouse_enabled(mouse_grabbed);
                self.control.set_cursor_grabbed(mouse_grabbed);
            } else if self.control.poll_gesture(&help_gesture) {
                current_help = current_help % 2 + 1;
                match current_help {
                    0 => self.text[short_help].set_visible(true),
                    1 => {
                        self.text[short_help].set_visible(false);
                        self.text[long_help].set_visible(true);
                    }
                    2 => self.text[long_help].set_visible(false),
                    _ => unreachable!(),
                }
            }

            self.player.update(delta, &self.control, &self.level);
            self.scene.set_modelview(&self.player.camera().modelview());
            self.scene.set_projection(self.player.camera().projection());
            self.level.update(delta, &mut self.scene);

            self.scene.render(&mut frame, delta)?;
            self.text.render(&mut frame)?;

            let updates_t1 = time::precise_time_s();
            cum_updates_time += updates_t1 - updates_t0;

            cum_time += f64::from(delta);
            num_frames += 1.0;
            if cum_time > 2.0 {
                let fps = num_frames / cum_time;
                let cpums = 1000.0 * cum_updates_time / num_frames;
                info!(
                    "Frame time: {:.2}ms ({:.2}ms cpu, FPS: {:.2})",
                    1000.0 / fps,
                    cpums,
                    fps
                );
                cum_time = 0.0;
                cum_updates_time = 0.0;
                num_frames = 0.0;
            }

            // TODO(cristicbz): Re-architect a little bit to support rebuilding the context.
            frame.finish().expect(
                "Cannot handle context loss currently :(",
            );
        }
        Ok(())
    }
}

const SHORT_HELP: &'static str = "Press 'h' for help.";
const LONG_HELP: &'static str = r"Use WASD or arrow keys to move and the mouse to aim.
Other keys:
    ESC - to quit
    SPACEBAR - jump
    ` - to toggle mouse grab (backtick)
    f - to toggle fly mode
    c - to toggle clipping (wall collisions)
    h - toggle this help message";
