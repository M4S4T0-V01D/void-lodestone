# Changelog

## 0.4.0 - 2026-11-14

- A RuneLite profile per character: turn it on in Settings › Characters and each character starts RuneLite with a profile named after it, made from your active profile the first time. A character can also use any profile you pick.
- Copy one RuneLite profile's settings over others (the old files are kept as `.bak`).
- Notes, goals, tags and a colour per character, from the new Notes button under Play. Tags and the colour show in the accounts list; the tags and the next goal show under Play.
- Game windows are titled with the character's name.
- Window layouts: Lodestone remembers where each character's window was and puts it back next time, and Tile lays the running games out over one monitor or all of them. It only moves windows; it never sends keys or clicks.
- News and status: Jagex's latest news for Old School and RuneScape 3, whether sign-in is up, Old School worlds online and player counts.
- Backup and restore in Settings › Backup: Lodestone's settings, groups, notes and playtime plus RuneLite's settings and profiles, in one file in Documents. Sign-in sessions are never backed up.
- Coming from Bolt: copy RuneLite's settings and profiles over from Bolt. See docs/coming-from-bolt.md.
- Roll a client back to the version before if an update breaks something; updates pause until you resume them.

## 0.3.0 - 2026-11-14

- Lodestone updates itself: it looks for new releases on GitHub at start-up and every six hours (turn it off in Settings), shows an Update button in the top bar, and installs the new Flatpak after checking its checksum. Restart when your games are done.
- What's new: the release notes appear once after an update.
- Keyboard shortcuts: Ctrl+Enter plays, Ctrl+, opens Settings.
- Settings opens and responds much faster: dialogs no longer blur the window behind them, animations pause while one is open, and Save closes at once.
- Background pictures load as raw bytes instead of a huge data URL, and the blur is drawn once into the picture rather than by the webview on every repaint.
- Accounts are no longer looked up in the keyring on every refresh.
- New appearance options with a live preview: accent colour, UI size (80–140%), compact spacing, corner roundness, hiding the logo and title, which part of the background stays in view, and how often screenshots rotate.
- Fold the accounts and running-games panels away from buttons in the top bar.
- The layout adapts to narrow windows and large UI sizes.
- Escape closes Settings from anywhere in the window.

## 0.2.0 - 2026-10-29

- Add an account in a Lodestone window or in your own browser.
- Launch several characters at once: add rows with **+ Add**, each with its own client (RuneLite or HDOS, normal or custom jar).
- Saved groups: name the current rows and start them with one click, from the window or the tray.
- Custom client jar (for example a RuneLite developer bootstrap), with a Normal/Custom switch next to Play and a button for the sideloaded plugins folder.
- Debug switch: starts the client with a JDWP port for your IDE to attach to.
- Build & play: builds your plugin projects with Gradle, copies the jars to the sideloaded plugins folder and starts (or restarts) your characters only if every build works.
- "Rebuilt, restart?" when the custom jar changes while the game runs.
- Running games: Stop all, Restart with the same character, client and jar.
- Live log viewer: jump between errors, folded stack traces with your own plugin's frames kept, copy and save.
- Crash notices with Open log and Restart, and desktop notifications for crashes, expired sessions and failed update checks.
- Tray icon listing running games, starting groups and stopping everything; optionally close to the tray while games run.
- Session and update health for each account and client.
- Playtime per character (today, this week, total), kept only on this computer.
- Background picture behind the middle area: your own picture per game, or rotating RuneLite screenshots, with darken and blur.
- Flatpak: includes a full Java compiler for Build & play; Documents is now writable for Gradle builds.

## 0.1.0 - 2026-10-25

First release.

- Sign in with multiple Jagex accounts; characters are listed per account, with rename, reorder and remove.
- Launch Old School RuneScape through RuneLite or HDOS, and RuneScape 3.
- Clients are downloaded from their official sources and kept up to date.
- Run several games at once; see uptime, open logs and stop them.
- Java options, client arguments, custom Java, launch wrappers and a game home folder.
- Seven VOID themes and a reduce-motion option.
- Game sessions are kept in the system keyring. No telemetry.
- Flatpak with Java and the RuneScape 3 libraries included.
