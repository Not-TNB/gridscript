**GridScript by SuperJedi224**

In GridScript, a rather unusual programming language, commands are assigned in any order to coordinates and then executed following a dynamically generated path through this virtual grid, stopping upon exiting the grid. Values can be stored as variables, in a buffer, or in a 2-dimensional dataspace.

The X-axis is numbered from left (WEST) to right (EAST), and the Y-axis from top (NORTH) to bottom (SOUTH).

The program starts with a title in all caps preceded by an octothorpe (#) and followed by a period. This is followed by two line breaks and then a list of metadata. This is followed by two more line breaks and then the body of the script in the form of a list of nodes at one node definition per line.

Each node definition consists of the initial position of the node as an ordered pair of integers, a colon, then the command to be executed when the program tracer enters the node.

Nodes must be centered over intersections of gridlines, but the program tracer is not always required to follow these gridlines.

## DATA TYPES & CASTING

There are four data types in the program: INT, FLOAT, STRING, and BOOL.

* INT is a 32-bit signed integer
* FLOAT is a 32-bit IEEE 754 float.
* STRING is a sequence of 8-bit characters of any length. Characters are raw bytes — any value from 0 to 255 is a valid character, and no encoding is assumed or validated on read.
* BOOL has two possible static values, labeled TRUE or FALSE.

The special value NULL is separate from the main types. It is assumed to be the value of any variable that has not yet been assigned a value, and may also arise in other situations, such as attempting to load from a nonexistent text file.

When casting from another type to an INT or FLOAT, the empty string, FALSE, and NULL are all cast to 0, and TRUE is cast to 1. When casting to an INT, FLOATs should be rounded down (towards 0). INTs can be cast to FLOATs without modifying the value. All other values cannot be cast to INTs or FLOATs.

A STRING is castable to FLOAT if, after trimming leading/trailing whitespace, it matches a standard signed decimal literal (optional `+`/`-`, digits, optional decimal point and fractional digits, optional exponent). Casting a STRING to INT reuses this exact same parse and then floors the result, mirroring the FLOAT→INT rounding rule above — there is only one numeric-string parser in the interpreter. A STRING that doesn't match this grammar cannot be cast to INT or FLOAT.

When casting from another type to a BOOL, the empty string, the string "0", the number 0 (as an int or a float), and NULL are cast to FALSE. All other values are cast to TRUE. (Note that only the literal string "0" is special-cased — the word "FALSE" is a non-empty string that isn't "0", and so casts to TRUE like any other non-empty string.)

Casting from another type to a string is fairly straightforward, with the exception that NULL should be cast to the empty string as opposed to the string "NULL".

## THE INTERPRETER

There are two tracers involved in a GridScript implementation: the Data Tracer, and the Program Tracer. The Data Tracer exists in the dataspace, and the Program Tracer exists in the program space.

The dataspace is a simple grid with dimensions supplied in the program metadata. Each grid cell contains a value of type INT, which is initially 0. The Data Tracer inhabits one of these cells at a time. It does not have an inherent direction. At the beginning of the program, the Data Tracer begins at the top left cell.

The program space, however, is not strictly a grid, but rather a continuous space with the precision of 32-bit floats, with dimensions again specified in the program metadata, in which the Program Tracer lives. The Program Tracer has an inherent position, consisting of an X-coordinate and a Y-coordinate which are 32-bit floats. It also has a direction, specified by an integer from 0 to 359 inclusive. The program tracer begins at the center of the node with command START and with direction 0.

The program space is inhabited by a number of *nodes*, which are circles centered at integral positions. The radius of these circles is provided by a parameter in the program metadata described below. Each node is associated with a command. If the Program Tracer enters one of these nodes, the command associated with that node is executed, then the program tracer exits the node in its current direction and continues until it reaches another node or the edge of the program space. A list of commands is specified in a later section.

The program space also contains *checkpoints*, which are found at integral positions just like nodes and are declared using the same `(x,y):CHECKPOINT id` syntax. A checkpoint is a static marker, not a node: it is recorded at load time, has no radius, is never "entered" by the Program Tracer, and never triggers execution of anything. It exists purely as a labeled coordinate for `GOTO` to target, and is available to `GOTO` from the very start of the program regardless of whether the tracer has ever passed near it. Each checkpoint has an inherent ID, which is a nonnegative integer. Multiple checkpoints may share an id, in which case `GOTO` picks the nearest one (see the GOTO command).

All objects that "exist" in the program space are bound to the program space's dimensions. That is, their X coordinates must be in the range [1,width] and their Y coordinates in [1,height]. (These parameters are specified in the metadata.) An exception is thrown if a node is initially centered outside this rectangle. If the Program Tracer exits the rectangle, then the program halts and returns an error code of 0 (no error). If this happens to a subroutine's tracer rather than the main program's, it is treated as an implicit `RETURN NULL` to the caller instead of halting the whole program; only the main program's tracer exiting the rectangle halts everything.

The program also has a buffer which can hold an arbitrary amount of data. Each datum can be any of the four types. It has an inherent "length" and thus behaves similarly to a list data structure. By default, values are added at the top and removed from the bottom (first-in-first-out); `TOP` is available as an explicit opt-in when a command needs last-in-first-out behavior instead.

Finally, there are two methods of output and one method of input. The first is the program output, which is stdout. The second is the debug output, the visibility of which is determined by the program's metadata. When visible, it shows a live view of the Program Tracer's position and path through program space, along with a running log of every warning and exception thrown during execution. If it is visible, it should be as a GUI. If no GUI is available, it should simply be ignored. The input is stdin.

## PROGRAM EXECUTION

The program execution is as follows:

1. Advance the Program Tracer in its current direction (this can be done by adding the cosine of its direction to its x position and the sine of its direction to its y position).
2. If the Program Tracer is outside of the program space's boundary, exit with an error code of zero (no error).
3. If the segment traced by the Program Tracer's movement this step intersects one or more nodes that it was not already inside at the start of the step (or if there was no previous step), execute the commands associated with those nodes in the order that they were written inside the program file.

This is repeated until an exception is thrown, or the program halts as in step 2.

The segment-intersection rule in step 3 applies only to the tracer's physical advance in step 1 — using it ensures a node can never be skipped over even if its radius is smaller than one step's travel distance. A `GOTO` (or any other instantaneous relocation) is not movement through the intervening space, and is checked differently: with a plain point-containment test at the destination only. If that point lies inside a node, that node's command executes; if not, nothing does.

The set of nodes triggering execution in a given step is a fixed snapshot taken once, at the moment step 3 begins for that step. If one of the resulting commands is a `GOTO` that moves the tracer elsewhere, the remaining commands already queued for that step still execute in file order — a mid-batch teleport does not cancel other nodes that were already triggered by that step's movement.

## METADATA

The syntax for a metadatum is as follows:

`@<key> <value>`

The following keys are accepted:

**width:** the value of this is an integer specifying the width of the grid, if this is not included, or less than 1, execution fails and an exception is thrown
**height:** the value of this is an integer specifying the height of the grid, if this is not included, or less than 1, execution fails and an exception is thrown
**datawidth:** the value of this is an integer specifying the width of the dataspace, defaults to 64, if this is less than 1, execution fails and an exception is thrown
**dataheight:** the value of this is an integer specifying the height of the dataspace, defaults to 64, if this is less than 1, execution fails and an exception is thrown
**radius:** the value of this is an integer specifying the radius of each node, defaults to 1, if this is less than 1, execution fails and an exception is thrown
**steps:** the value of this is an integer; the program immediately halts and throws an exception if it runs for more than this many steps without halting, if this is less than 1, execution fails and an exception is thrown immediately. If this is omitted, the program will be allowed to run indefinitely, that is, no maximum number of steps will be enforced. Each subroutine instance gets its own fresh `steps` budget, so this alone does not bound total recursive work across nested CALLs — see `maxdepth`.
**debug:** the value can be any of the following: 'true' 'false' 'auto'. If true; the debug console always shows. If false; the debug console never shows. If auto; the console starts hidden but will show when any exception or warning is thrown. Defaults to 'false'
**seed:** an integer seed for the interpreter's shared random source, used by `GO RANDOM`, `SWITCH RANDOM`, `STORE RANDOM`, `REMOVE ANY POSITION`, `SHUFFLE`, and GOTO tie-breaking. If omitted, a nondeterministic system seed is used.
**maxdepth:** an integer specifying the maximum nesting depth of CALL (i.e. how many subroutine calls may be active at once across the entire program, including nested/recursive calls). Defaults to 1000. An exception is thrown if this is exceeded. This is read once from the main program's metadata only — it is a property of the whole active call stack, not something individual subroutines can override, so subroutine-level `@maxdepth` declarations, if present, are ignored.

## COMMANDS

These are the commands that may be assigned to nodes.
A node definition takes the form (x,y):COMMAND.

**START**
The main program and each subroutine must contain exactly one of these nodes (or an exception is thrown). As described above, the Program Tracer starts here with direction 0.

**GO NORTH|SOUTH|EAST|WEST|RANDOM|[RELATIVE TO] THIS DIRECTION|[RELATIVE TO] direction**
Set the Program Tracer's direction clockwise from EAST. NORTH sets direction to 270, SOUTH to 90, EAST to 0, and WEST to 180. RANDOM selects a random integer from 0 to 359 inclusive. THIS DIRECTION uses the value at the position in the dataspace as indicated by the Data Tracer, taken modulo 360. If a numerical value is provided, it is cast to an integer, and the direction of the Program Tracer is set to that value modulo 360.

RELATIVE TO THIS DIRECTION is the same as THIS DIRECTION, but the stored value is added to the Program Tracer's direction and taken modulo 360. Finally, if RELATIVE TO is followed by a numerical value, that is cast to an integer, added to the current direction, and taken modulo 360.

**CHECKPOINT id**
Declares a checkpoint marker with the specified id, located at the center of the CHECKPOINT node. ID must be a non-negative integer, or an exception is thrown. This is a load-time declaration, not a runtime-triggered command — see "checkpoints" above.

**GOTO id|THIS CHECKPOINT**
Teleports the program flow to the nearest checkpoint having the specified id, if one exists. If multiple such checkpoints are at equal distances, it chooses one at random (from the shared seeded RNG). Does not change the direction of the flow. If THIS CHECKPOINT is used, it uses the number at the current position in the Dataspace. If ID is a non-integer, or no such checkpoint exists, a warning is thrown and the tracer's position is unchanged.

**SWITCH RANDOM|value|!value|=value|!=value**
value: Rotates the direction of the program tracer 90 degrees clockwise if the specified value is or can be cast to TRUE, else leaves the direction of the program tracer unchanged
!value: Rotates the direction of the program tracer 90 degrees clockwise if the specified value is or can be cast to FALSE, else leaves the direction of the program tracer unchanged
=value: Rotates the direction of the program tracer 90 degrees clockwise if the current dataspace value equals the specified value, else leaves the direction of the program tracer unchanged
!=value: Rotates the program tracer's direction 90 degrees clockwise if the current dataspace value does not equal the specified value, else leaves the direction of the program tracer unchanged
RANDOM: Has a 50% chance of rotating the program tracer's direction 90 degrees clockwise, else leaves the direction of the program tracer unchanged

**STORE value|RANDOM [TO variable]**
Stores the specified value to the specified variable, if applicable. Else, stores it at the current dataspace value. If RANDOM is used, a random floating point number between 0 and 1 (including 0, but not 1) is used. The variable name may contain any combination of lowercase letters, underscores, and digits, of which the first character may not be a digit.

**INPUT [type] [variable] [WITH PROMPT prompt]**
Takes a value from the input, storing it to the specified variable if applicable, or else at the current position in the Dataspace. If a prompt is specified, it will send that message to the input and use the response as the value stored. If *type* is specified, it will check to see if the data is or can be cast to the specified type before storing; if not, it throws a warning and stores NULL instead. *Type* may be either INT, STRING, FLOAT, or BOOL. Should return NULL on end of file.

When used in a subroutine, prompt is ignored and it takes values from the subroutine call's argument list, consumed in order (each INPUT call consumes the next not-yet-consumed argument). The same type-check-and-cast rule applies as above — if the argument is not or cannot be cast to the requested type, a warning is thrown and NULL is stored instead. An exception is thrown if a subroutine attempts to take more inputs than the argument list gives it.

**LOAD FILE path [TO [type] variable]**
As INPUT, but it takes the contents of the specified file as the input. If the file does not exist, or the contents cannot be properly cast, a warning is thrown and it stores NULL instead.
The file may be of any plaintext type.

**NEXT VALUE**
Moves the data tracer one position right, wrapping to the beginning of next row (or the first row if it's already at the bottom) if it reaches the right edge

**PREVIOUS VALUE**
Moves the data tracer one position left, wrapping to the end of previous row (or the last row if it's already at the top) if it reaches the left edge

**NEXT ROW**
Moves the data tracer to the beginning of the next row, wrapping to the top row if it's already at the bottom

**PREVIOUS ROW**
Moves the data tracer to the beginning of the previous row, wrapping to the bottom row if it's already at the top

**PRINT [value|NEWLINE|IMAGE path|FILE path]**
Casts the specified value, or else the current value in the dataspace, to a STRING and prints it on the current line of the output console.
If NEWLINE is specified instead, it skips to the next line of the output console.
If IMAGE is used, it displays the specified image on the output console, preceded and followed by line breaks. If the output console does not support graphical output, a warning is thrown and the filename is printed to the console instead. If the image does not exist, a warning is thrown and the output console is unchanged.
If FILE is used, it displays the contents of the specified file (which may be of any plaintext type) on the output console, preceded and followed by line breaks. If the textfile does not exist, a warning is thrown and the output console is unchanged.

**PUSH [value]**
Pushes either the specified value, or else the current value in the dataspace, to the top of buffer.

**REMOVE [position|THIS POSITION|TOP|ANY POSITION] [TO [type] variable]**
Removes a value from the buffer and stores it to the specified variable (after casting to the specified type, if applicable), if applicable, or else to the current position in the dataspace. In the latter case, it will first be cast to an INT, if possible, otherwise 0 will be stored and a warning is thrown.
If a numerical position is specified, it takes from that position, starting with 0 as the bottom of the buffer. (An out-of-range or non-integer position will throw a warning, and store NULL instead)
If THIS POSITION is used, it uses the current dataspace value as the position.
If TOP is used, it takes from the top.
If ANY POSITION is used, it takes a buffer value at random (from the shared seeded RNG).
Otherwise, it just takes from the bottom.

**PEEK [TO [type] variable]**
Copies the bottom buffer value and, if a target variable is specified, stores it there (after casting to the specified type, if applicable). Otherwise, if possible, casts it to a number and stores it to the current position in the dataspace; if not possible, a warning is thrown.
Note that this does not modify the buffer in any way.

**HOME**
Sends the data tracer back to the top left corner of the dataspace

**MOVE LAST NODE TO|BY x y**
Moves the last node the program tracer went through as specified (TO moves it to the specified coordinates, BY moves it relative to its current coordinates); note that this only applies for the current execution; if the node is moved out of bounds it should be deleted for the remainder of the current execution. "Last node" is tracked independently per program-space instance — the main program and each subroutine call track their own, unaffected by nodes visited in a called or calling subroutine's own space. Since checkpoints have no radius and are never "entered," they can never be the target of this command.

**INCREMENT [variable1] [BY value] [GIVING variable2]**
Increases variable1, or else the value at the current position in the dataspace, by the specified value, or else by 1. If variable2 is specified the original value is not modified and the result is instead stored in variable 2. If the result overflows the 32-bit INT range, an exception is thrown.

**DECREMENT [variable1] [BY value] [GIVING variable2]**
As Increment, except it decreases the value instead of increasing it. Overflow behaves as above.

**MULTIPLY [variable1] BY value [GIVING variable2]**
Multiplies variable1, or else the value at current position in the dataspace, by the specified value. If variable2 is specified the original value is not modified and the result is instead stored in variable 2. If the result overflows the 32-bit INT range, an exception is thrown.

**DIVIDE [variable1] BY value [GIVING variable2]**
Divides variable1, or else the value at current position in the dataspace, by the specified value. If variable2 is specified the original value is not modified and the result is instead stored in variable 2. If the divisor is 0, an exception is thrown.

**SHUFFLE**
Rearranges the buffer in a random order (from the shared seeded RNG)

**CALL name [WITH ARGUMENTS arguments] [GIVING variable]**
Calls the subroutine (more on this in the next section) with the specified name, using the specified list of arguments if applicable. Any return value will be stored to the specified variable, or else to the current position in the dataspace. Example: 'CALL FOO WITH ARGUMENTS "Foo" 17 GIVING foo'. If no value is returned, the dataspace should remain unmodified; but if storing to a variable instead and no value is returned, the variable should be set to NULL. An exception is thrown if more arguments are provided than the subroutine ever consumes via INPUT, or if fewer arguments are given than the subroutine requests. An exception is also thrown if the call would exceed `@maxdepth` active nested CALLs.

**SPLIT string [OVER separator]**
Splits the specified STRING value into one or more substrings over the specified separator string, if applicable, or else over any whitespace characters. Then, pushes all of the resulting substrings to the buffer.

**THROW message**
Throws an exception with the specified message.

**WARN message**
Throws a warning with the specified message

## Subroutines

Any subroutines should be defined after the main program, with two line breaks between each and two line breaks between the main program and the first subroutine. The main differences are:

1. Subroutine titles are preceded by two octothorpes instead of one
2. The INPUT command takes inputs from the argument list for that subroutine call instead of from the main input; inputs are taken in the order they are listed in the call
3. The RETURN command is added. It halts the subroutine, but not the main program, returning the specified value, if any, or otherwise the value at the current position in that subroutine instance's dataspace. If the subroutine's tracer instead exits its program space's boundary without ever executing RETURN, this is treated identically to `RETURN` with no value — i.e. an implicit `RETURN NULL` to the caller.

Each instance of a subroutine has a separate program space, set of variables, dataspace, and buffer from other subroutines, other instances of that subroutine, and the main program. The only ways to share values between them are with call arguments and return values. Metadata in subroutines should fall back first to the metadata for the main program before falling back to the default; this applies to `steps` (each instance gets its own fresh budget). `maxdepth` is a single global setting read from the main program only (see METADATA above) and is not subject to this fallback.

CALL is synchronous: the calling tracer's execution pauses at the CALL node until the subroutine runs to completion (via RETURN or implicit RETURN NULL) before the caller continues.

## Dynamic Variable Naming

Anywhere the grammar accepts the name of a variable to write to, or a value that can be read from a variable, the phrase "THE VARIABLE NAMED *name*" may be substituted, where *name* is a STRING containing any valid variable name. This allows variable names to be assigned dynamically as well as values, and applies uniformly across every command. Additionally, wherever this substitution is valid, *type* THE VARIABLE NAMED *name* may be written as "THE *type* NAMED *name*" (*type* is, as usual, one of INT, FLOAT, STRING, or BOOL).

## Comments

A pair of exclamation points (!!) indicates that the rest of the line is a comment

## Grammar Notes

The command grammar is English-phrase-like but resolvable without ambiguity via keyword lookahead, since all reserved words are UPPERCASE and all variable names are lowercase-only (letters, underscores, digits; first character not a digit) — so a bare identifier token can never be confused with a keyword. Informally:

```
value-expr    ::= literal | var-ref
var-ref       ::= identifier
                 | "THE" "VARIABLE" "NAMED" string-literal
                 | "THE" type "NAMED" string-literal
literal       ::= int-literal | float-literal | string-literal | "TRUE" | "FALSE"
type          ::= "INT" | "FLOAT" | "STRING" | "BOOL"
argument-list ::= value-expr (value-expr)*   ; space-separated, self-delimiting
                                              ; by the lookahead rule above
```

This grammar underlies `CALL ... WITH ARGUMENTS ...`, and every other command's value/variable-name arguments.

## SAMPLE PROGRAMS

**Hello World!**
```
#HELLO WORLD.

@width 4
@height 1

(1,1):START
(3,1):PRINT 'Hello World'
```

**TRUTH MACHINE**
```
#TRUTH MACHINE.

@width 10
@height 6

(1,1):START
(3,1):INPUT INT
(5,1):GO EAST
(7,1):SWITCH =1
(7,3):PRINT 1
(7,5):GO WEST
(5,5):GO NORTH
(9,1):PRINT 0
```

**(Alternate Version)**
```
#TRUTH MACHINE 2.

@width 8
@height 8

(1,1):START
(3,1):INPUT
(5,1):SWITCH =1
(5,3):CHECKPOINT 0
(5,5):PRINT 1
(5,7):GOTO 0
(7,1):PRINT 0
```

**Factorial** *(valid for n ≤ 12 — MULTIPLY throws on 32-bit INT overflow beyond that)*
```
#FACTORIAL.

@width 14
@height 8

(1,3):START
(7,1):CHECKPOINT 0
(3,3):INPUT INT TO n
(5,3):STORE n
(7,3):GO EAST
(9,3):DECREMENT n
(11,3):SWITCH n
(11,5):MULTIPLY BY n
(11,7):GOTO 0
(13,3):PRINT
```

**Ackermann Function**
```
#ACKERMANN FUNCTION.

@width 9
@height 1

(1,1):START
(3,1):INPUT INT TO x
(5,1):INPUT INT TO y
(7,1):CALL ACK WITH ARGUMENTS x y
(9,1):PRINT

##ACK.

@width 19
@height 7

(1,1):START
(3,1):INPUT INT TO x
(5,1):INPUT INT TO y
(7,1):SWITCH !x
(7,3):INCREMENT y
(7,5):RETURN y
(9,1):SWITCH !y
(9,3):DECREMENT x
(9,5):CALL ACK WITH ARGUMENTS x 1
(9,7):RETURN
(11,1):DECREMENT y
(13,1):CALL ACK WITH ARGUMENTS x y GIVING z
(15,1):DECREMENT x
(17,1):CALL ACK WITH ARGUMENTS x z
(19,1):RETURN
```