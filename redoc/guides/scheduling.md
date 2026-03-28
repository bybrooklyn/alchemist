# Scheduling

Automating your transcoding with Alchemist's scheduler.

The **Scheduler** lets you decide *when* Alchemist is allowed to work. This is perfect if you want Alchemist to run while you're asleep but stop during the day so it doesn't slow down your internet or your games.

## Setting a Schedule

Go to **Settings** > **Schedule** to tell Alchemist when to work.

### Creating a "Work Window"
You define a start and end time for Alchemist.
- **Start Time:** When Alchemist can start shrinking files (e.g., 11:00 PM).
- **End Time:** When Alchemist must stop and take a break (e.g., 7:00 AM).
- **Days:** You can set different schedules for weekdays and weekends.

### Can I have more than one?
Yes! You can have Alchemist work at night during the week, but stay active all day on Sunday while you're out of the house.

## What happens when time runs out?

If Alchemist is in the middle of shrinking a movie and the "work window" ends, it will **pause**. It saves its spot and will pick up exactly where it left off the next time the clock hits your start time.

## Manual Overrides

Even if you have a schedule set, you can always tell Alchemist to "Work Now" by clicking the **Force Start** button in the Job Manager. This is useful if you know you're going to be away from your computer for a few hours.

## Frequently Asked Questions

**Does pausing ruin the file?**
No. Alchemist is very careful. It pauses FFmpeg (the engine) safely so the file stays perfectly fine.

**Will it wake up my computer?**
Alchemist needs the computer to be turned on to work. It won't wake a "sleeping" computer, so make sure your power settings allow the computer to stay awake during the work window.
