# Background for Realm Writers
The primary goal of Spadina is to make it easier for writers to create new
realms. Creating game assets is hard, so this document will address creating
realms two parts: the built-in functionality of creating a realm and creating a
new game asset (_e.g._, 3D models).

All assets that have been uploaded to the Spadina federated network get an
identifier that is a long string of numbers and letters. An asset can be pulled
included into the development of any realm. The goal is for work done by some
artists to be reusable by others. To this end, realms are not modelled as
complete worlds, but as small pieces that are assembled and the pieces can be
reused.

Currently, all game assets need to be licensed in a way that allows others to
use them: a [Creative Commons](https://creativecommons.org/) or public domain
licences.

## Distribution, Instances, and Links
When a realm is created, the assets will be uploaded to your server and other
servers can get copies of your realm and its assets. Once a realm is available
in the network, any player can create their own instance using the realm's
asset identifier. Every realm instance that is created also has an identifier
and accessing that realm requires that server to be online.

Updating a published realm is not possible. It is possible to create a brand
new realm and players can create an instance of the new realm. When creating a
realm, the _editor_ is used that can allow playing a realm before publishing
it.

Realms can contain references to other realms. These are called _links_ both
because that's what _Myst_ calls them and they are effectively hyperlinks.
There are there are a few kinds:

- spawn point: a different location in the same realm
- nowhere: removes the player from this world to no specific place
- settings: the player can select a target location. This is effectively a
  fill-in-the-blank link, where the owner of the instance gets to choose the
  destination.

## Client/Server Split and Puzzles
When creating a realm, some of the behaviour will exist on the client and some
will exist on the server. This mostly doesn't concern realm creators, but the
following should be noted:

- the state of the puzzles is hidden on the server, so no player can extract puzzle information in order to cheat. Only things that you have made visible are visible to players.
- there is a limit to how much processing the server allows for a puzzle. If the puzzle is programmed poorly, the server will cut off the puzzle.

For the puzzle cut-off, this means:

- there is an upper limit for how large and complex a puzzle can be
- puzzles shouldn't have "unstable" behaviour where they oscillate rapidly between states

In 3D (and even 2D) graphics, objects have _properties_, _attributes_, or
_modifiers_ that determine their appearance, such as colour, albedo, and
transparency, that can be set to different values. Spadina's design is that
every property can be set from the game logic. There are two selection modes:
set based on a Boolean (true/false; on/off) value or set based on a number.

In addition to allowing properties to be set from the puzzles, it is also
possible to select randomly from pre-determined values or lets players choose.

## State Machines
The default way to to program the puzzles is using state machines. A state
machine can be thought of as a table with the columns being different
properties of the world to control. When a world is created, the state machine
will _select_ the first row in the table. The rows can have _transitions_ which
cause the state machine to select a new state based on how players interact
with the world (or a timer).

Let's start with a simple state machine to control a self-closing door:

| State | Door    | Timer |
|-------|---------|-------|
| 1     | Closed  | Off   |
| 2     | Open    | 30sec |

| State | Action        | New State |
|-------|---------------|-----------|
| 1     | Click Button  | 2         |
| 2     | Timer Elapsed | 1         |

This would create a door that would:

- start closed
- open when the button is pushed
- close automatically after 30 seconds

Let's suppose we want to create a door that needs the button to be pushed
twice, with a 30-60 second delay, and then remain open until the button is
pushed again:

| State     | Door    | Timer |
|-----------|---------|-------|
| Closed    | Closed  | Off   |
| Triggered | Closed  | 30sec |
| Waiting   | Closed  | 30sec |
| Open      | Open    | Off   |

| State      | Action        | New State |
|------------|---------------|-----------|
| Closed     | Click Button  | Triggered |
| Triggered  | Click Button  | Closed    |
| Triggered  | Timer Elapsed | Waiting   |
| Waiting    | Click Button  | Open      |
| Waiting    | Timer Elapsed | Closed    |
| Open       | Click Button  | Closed    |

## Area Interaction
While the state machines can be triggered by direct player action (_i.e._,
click on things in the world), it is also helpful to have them interact by
indirect player action (_i.e._, moving around the world). Special _areas_ can
be marked in the world and the puzzle can count the number of players in that
area and the puzzle can affect players in those areas.

As players move around the world, the number of players occupying an area is
available as an action and it can be used in a mathematical formula (_e.g._,
number of players greater than 3). It is also possible to use combinations of
players counts (_e.g._, number of players in area A is greater than number of
players in area B).

The puzzle can then set the behaviour of an area when a player enters it:

- normal - players can pass through this area
- tranfer players from one area to another
- link players to a realm

Although there is no inventory, realms have the ability to _mark_ players. Each
player has a number of bits (switches) that can be turned on and off. The
behaviour of an area and the counts available to the puzzle can be filtered
based on these switches.

For example, suppose there is a forbidden doorway near the entrance of a realm.
If players enter the doorway, they get sent home. Players need to travel to
another room and solve a puzzle. When that puzzle is solved correctly, and
players in that room are marked. Then, when marked, players can pass through
the forbidden doorway.

Marking can also affect how the world is displayed to the player. For instance,
suppose you want to create a puzzle where two players have to collaborate to
solve a puzzle. The world could be designed so that each player has to solve a
puzzle to get a different mark. Depending on which mark they have, they see a
different button light up in the world. They then have to press the right pair
of buttons based on their combined knowledge.
The puzzle can also _mark_ players
