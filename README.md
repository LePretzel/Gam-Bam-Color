# Gam-Bam-Color
A simple Gameboy Color emulator written in Rust. I mainly started this as a fun project to build whlile learning Rust, so it's focus isn't on being the most feature complete or providing the best gameplay experience. However, most games I've tested are fairly playable although many have some minor graphical glitches and a few that depend on more obscure behavior don't work at all.

## Building
The project depends on SDL2 and the Rust toolchain but once those are installed, just download the source code and execute ```cargo run --release```. I've included Blargg's test roms which will automatically be loaded and executed when the project is built. 

## Controls
In case you want test the emulator on another rom, the controls are as follows:
- A => X
- B => Z
- D-pad => Arrow Keys
- Start => Enter
- Select => Backspace
- Close => Esc
- Throttle/Unlock Framerate => Space Bar

## Images
<img width="800" height="720" alt="2025-09-08-231714_hyprshot" src="https://github.com/user-attachments/assets/5a88a007-e93e-42cd-9440-4e99d8b246d3" />
<img width="800" height="720" alt="2025-09-08-231210_hyprshot" src="https://github.com/user-attachments/assets/3670e0f2-c28e-4061-8b90-6f87d1348490" />
<img width="800" height="720" alt="2025-09-08-231232_hyprshot" src="https://github.com/user-attachments/assets/89c8e7a4-28f7-4f91-89b4-ece4a613cbb8" />
<img width="800" height="720" alt="2025-09-08-231346_hyprshot" src="https://github.com/user-attachments/assets/9c19c46d-6a75-42cc-a044-08e1b6db22b8" />
<img width="800" height="720" alt="2025-09-08-231839_hyprshot" src="https://github.com/user-attachments/assets/2ecaabfb-0489-4cc7-a42a-06eb7950004e" />

