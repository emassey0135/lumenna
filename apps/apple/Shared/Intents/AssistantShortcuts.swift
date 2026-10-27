import AppIntents

/// What Siri, Spotlight, the Shortcuts app and the Action Button offer without any setting up:
/// the operations used most, each named as the core names it (an operation's title, or the
/// launch shortcut's it stands for: New Task, Today). The phrases are Siri's, which the core has
/// none of yet.
struct LumennaShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: AddTaskIntent(),
            phrases: [
                "Add a task in \(.applicationName)",
                "New task in \(.applicationName)",
                "Add to \(.applicationName)",
                "Remember something in \(.applicationName)",
            ],
            shortTitle: "New Task",
            systemImageName: "plus"
        )
        AppShortcut(
            intent: WhatShouldIWorkOnIntent(),
            phrases: [
                "What should I work on in \(.applicationName)",
                "What's next in \(.applicationName)",
            ],
            shortTitle: "What Should I Work On?",
            systemImageName: "arrow.forward.circle"
        )
        AppShortcut(
            intent: StartOnIntent(),
            phrases: [
                "Start working in \(.applicationName)",
                "Start on something in \(.applicationName)",
            ],
            shortTitle: "Start on a Choice",
            systemImageName: "play.circle"
        )
        AppShortcut(
            intent: ToggleTimerIntent(),
            phrases: [
                "Start the timer in \(.applicationName)",
                "Start or stop the timer in \(.applicationName)",
                "Toggle the timer in \(.applicationName)",
            ],
            shortTitle: "Start or Stop the Timer",
            systemImageName: "playpause"
        )
        AppShortcut(
            intent: ShowTimerIntent(),
            phrases: [
                "Show the timer in \(.applicationName)",
                "How long have I been working in \(.applicationName)",
            ],
            shortTitle: "Show the Timer",
            systemImageName: "timer"
        )
        AppShortcut(
            intent: StartTimerIntent(),
            phrases: [
                "Start a timer in \(.applicationName)",
            ],
            shortTitle: "Start a Timer",
            systemImageName: "play"
        )
        AppShortcut(
            intent: PauseTimerIntent(),
            phrases: [
                "Pause the timer in \(.applicationName)",
                "Pause my timer in \(.applicationName)",
            ],
            shortTitle: "Pause a Timer",
            systemImageName: "pause"
        )
        AppShortcut(
            intent: StopTimerIntent(),
            phrases: [
                "Stop the timer in \(.applicationName)",
                "Stop my timer in \(.applicationName)",
            ],
            shortTitle: "Stop a Timer",
            systemImageName: "stop"
        )
        AppShortcut(
            intent: ShowDayIntent(),
            phrases: [
                "Show today in \(.applicationName)",
                "Show my day in \(.applicationName)",
                "What's on today in \(.applicationName)",
            ],
            shortTitle: "Today",
            systemImageName: "calendar"
        )
        AppShortcut(
            intent: CompleteTaskIntent(),
            phrases: [
                "Complete a task in \(.applicationName)",
                "Complete \(\.$id) in \(.applicationName)",
                "Mark \(\.$id) done in \(.applicationName)",
            ],
            shortTitle: "Complete a Task",
            systemImageName: "checkmark.circle"
        )
    }
}
