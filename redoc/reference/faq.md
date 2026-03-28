# FAQ

Frequently asked questions about Alchemist.

This page answers the most common questions people have about Alchemist. Whether you're wondering if it's free or how it handles your 4K movies, you'll find the answers here in plain English.

## General Questions

### 1. What exactly does Alchemist do?
Think of Alchemist as a "garbage compactor" for your video files. It takes large, older video files and converts them into modern formats (like HEVC or AV1) that take up much less space while keeping the same picture quality.

### 2. Is Alchemist free?
Yes! Alchemist is completely free and open-source. It is released under the GPLv3 license, which means the code is public and belongs to the community.

### 3. Will Alchemist ruin my video quality?
No. Alchemist is designed with "Intelligent Analysis." Before it even starts, it checks if the video is already "small enough." If it thinks a transcode would make the video look bad (what we call "quality murder"), it will skip that file automatically.

### 4. How much space will I actually save?
On average, users see between 30% and 70% savings. For example, a 10GB movie could shrink to 4GB or 5GB without any noticeable change in how it looks on your TV.

### 5. Does it work on Windows, Mac, and Linux?
Yes. Since Alchemist runs inside Docker, it works on almost any computer. We recommend Linux for the best performance with graphics cards, but it works great on Windows and Mac too.

### 6. Do I need a powerful graphics card?
You don't *need* one, but it helps a lot. A graphics card (GPU) can shrink a movie in 20 minutes, while a standard processor (CPU) might take 5 hours. Alchemist works with NVIDIA, Intel, AMD, and Apple Silicon.

### 7. What is the "Library Doctor"?
The Library Doctor is a feature that scans your existing movies to see if any of them are "broken" or corrupt. If it finds a file that won't play correctly, it can alert you or try to fix it.

### 8. Can I limit when Alchemist runs?
Yes. You can set a "Schedule" so Alchemist only works at night or when you aren't using your computer for gaming or work.

### 9. What happens to my original files?
By default, Alchemist keeps your original file and creates a new one with "-alchemist" in the name. You can change the settings to automatically delete the original file once the new one is verified to be good.

### 10. Does Alchemist support 4K and HDR?
Yes. Alchemist can handle 4K videos and preserves HDR (High Dynamic Range) metadata so your colors stay vibrant and bright on compatible TVs.

## Advanced Questions

### 11. What is "BPP" and why should I care?
BPP stands for "Bits Per Pixel." It's a math formula Alchemist uses to measure how much data is being used for every pixel of the video. It's the most reliable way to tell if a video is "high quality" or "highly compressed."

### 12. Can I use more than one graphics card?
Currently, Alchemist uses one primary graphics card for transcoding. You can select which one to use in the settings if you have multiple cards.

### 13. What is VMAF and should I enable it?
VMAF is a high-end tool developed by Netflix to "score" how a video looks compared to the original. It's very accurate but very slow. Only enable it if you are a "quality enthusiast" and don't mind transcodes taking much longer.

### 14. Can Alchemist handle subtitles?
Yes. You can choose to copy all subtitles to the new file, "burn" them into the video (so they are always visible), or ignore them entirely.

### 15. How do I update Alchemist?
If you are using Docker Compose, just run `docker compose pull` and then `docker compose up -d`. Your settings and history will be preserved.

### 16. What if my hardware isn't supported?
Alchemist will automatically "fall back" to using your CPU (Software Encoding). It will still work and produce great quality, it will just be much slower and make your computer fans spin fast!

### 17. Can I use Alchemist with Plex, Jellyfin, or Emby?
Absolutely. Alchemist is designed to work alongside your favorite media server. Just point Alchemist at your media folders, and it will update the files in place.
