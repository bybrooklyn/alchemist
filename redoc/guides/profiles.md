# Profiles

Customizing transcoding profiles for your media.

Think of "Profiles" as a set of rules for how Alchemist should shrink your videos. You can have one set of rules for high-quality movies and another for old TV shows.

Transcoding profiles define how Alchemist handles your media. You can choose from built-in profiles to match your specific needs.

## Built-in Profiles

Alchemist comes with several built-in profiles:

| Profile | Description | Best For |
| :--- | :--- | :--- |
| **Quality First** | Focuses on keeping every detail perfect. Files will be larger but look amazing. | 4K Movies and your favorites. |
| **Balanced** | The "just right" setting for most people. Good size and great quality. | Standard movies and TV shows. |
| **Space Saver** | Focuses on saving the most disk space possible. | Shows you've already seen or don't care about the tiny details. |
| **Streaming** | Makes sure the file plays smoothly on any device, even over slow Wi-Fi. | Watching on phones or tablets. |

## Assigning Profiles to Folders

When you tell Alchemist about a folder (like `/media/movies`), you pick a profile for it. This lets you treat different parts of your library differently.

### Example
- Your **Movies** folder uses **Quality First**.
- Your **TV Shows** folder uses **Balanced**.
- Your **Backups** folder uses **Space Saver**.

## What's inside a Profile?

If you want to get technical, each profile controls:
- **Codec:** Which "language" the video is written in (AV1, HEVC, or H.264).
- **Speed:** How hard the computer works. Faster is... well, faster, but slower usually makes smaller files.
- **Subtitles:** Whether to keep them, remove them, or "burn" them into the picture.
- **HDR:** How to handle those super-bright colors on modern TVs.

## Smart Skipping

Profiles also tell Alchemist when to *stop*. If a file is already smaller or better quality than what the profile would produce, Alchemist will skip it automatically to save time and electricity.
