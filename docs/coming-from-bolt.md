# Coming from Bolt

Lodestone can bring your RuneLite settings over from [Bolt](https://codeberg.org/Adamcake/Bolt): plugins, their settings, layouts, key bindings and every RuneLite profile. Your Jagex accounts are added again in Lodestone; Bolt's saved sign-in is never read.

## What gets copied

- RuneLite's `settings.properties`
- Everything in RuneLite's `profiles2` folder (the profiles and `profiles.json`)

Nothing else is read: not Bolt's `creds` file, not its browser cache, not RuneLite's `credentials.properties`. The files Lodestone already had are kept beside the new ones in a `before-restore-<date>` folder, so the import can be undone by hand.

Close RuneLite before importing. Lodestone refuses to import while it runs, because RuneLite writes its profiles when it closes and would put the old ones back.

## Where Bolt keeps them

| Bolt install | Folder |
| --- | --- |
| Flatpak (`com.adamcake.Bolt`) | `~/.var/app/com.adamcake.Bolt/data/bolt-launcher` |
| Native | `~/.local/share/bolt-launcher` |

Inside, RuneLite's folder is `.runelite`. You can point Lodestone at the Bolt folder or at `.runelite` itself.

## Steps

1. Close RuneLite in Bolt.
2. **Lodestone Flatpak only:** the Flatpak can't see other apps' folders, so copy Bolt's first. In a terminal:

   ```sh
   cp -r ~/.var/app/com.adamcake.Bolt/data/bolt-launcher ~/Documents/
   ```

   (Use `~/.local/share/bolt-launcher` instead if Bolt isn't a Flatpak.) You can delete the copy after the import, but be aware it contains Bolt's `creds` file too.
3. In Lodestone, open **Settings › Backup**. Under **Coming from Bolt**, pick the folder Lodestone found (or type its path) and press **Copy RuneLite settings from Bolt**.
4. Add your Jagex accounts with **Add account** and play.

## Profiles per character

Bolt starts every character with RuneLite's active profile. If you'd like each character to have its own, turn on **Settings › Characters › A RuneLite profile per character**: each character's first start makes a profile named after it from your active one. **Copy the settings of** on the same tab copies one profile over others later, and a character's **Notes** let it use any profile you choose.
