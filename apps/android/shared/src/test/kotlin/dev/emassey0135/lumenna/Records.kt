package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Asks
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.SidebarEntry
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.Subject

// The core's records, built by hand: on the JVM there is no core to ask for them.

fun offered(kind: ActionKind) = Offered(kind, Subject.PROJECT, kind.name, kind.name, false, false, null, Asks.NOTHING)

/** A heading of the places, open unless [folded]. */
fun group(group: SidebarGroup, folded: Boolean = false) = SidebarEntry(
    SidebarKind.Group(group), group.name, group.name, group.name, "", 0u, false, null, emptyList(),
    listOf(offered(if (folded) ActionKind.EXPAND else ActionKind.COLLAPSE)), false, !folded, null,
)

/** A project at [depth]; with something under it, [folded] or open. */
fun project(name: String, depth: UInt, folded: Boolean? = null) = SidebarEntry(
    SidebarKind.Place(dev.emassey0135.lumenna.app.Place.Project(name)), "project/$name", name, name, "", depth, false, null, emptyList(),
    when (folded) {
        null -> emptyList()
        true -> listOf(offered(ActionKind.EXPAND))
        false -> listOf(offered(ActionKind.COLLAPSE))
    },
    false, folded?.not(), null,
)
