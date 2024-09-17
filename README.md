# Reverse File

This is a program that takes printable input and reverses both word order and line order as an exercise. To make the exercise more challenging, I decided to assume limited buffer space and arbitrarily-long input.

I thought the exercise would be relatively easy compared to other projects I've completed, but this actually took some time and reiteration. There were multiple sub-problems to solve:

**Problem 1**

Because I assumed arbitrary input length, I couldn't assume that all of the input could be stored in memory and processed at once. This meant that I couldn't utilize the abstractions such as line iteration that normally trivialize the solution. To solve this issue, I had to process the input in batches. Because the input buffer was finite in size, I had to contend with the idea that a given read could include incomplete semantic data. Meaning, a given read could fill the buffer with only part of a line or even part of a word. Therefore, in addition to the read buffer, I needed a secondary storage space that I could consider practically infinite in capacity (compared to the buffer). This space stored partial words/lines.

**Problem 2**

In order to reverse the word order of a line, we have to be able to find a way to shift the words around. Files are "first-in, first-out" structures that can be truncated, appended to, or overwritten; they can't be inserted into (because they are specified with finite memory chunks?). Also, given that I assumed arbitrarily-long input, I could process the entire file and read it backwards. I had to find a way to insert each subsequent word at the head of a file, expanding it in the process.

I wrote a function to do so, allowing me to insert data at any specified offset within the file.

**Problem 3**

Because line and word order are reversed, the placement of space and newline had to be adjusted in order to result in the correct output. In other words, given an unmodified line, the space following the first word had to be moved to the last. Similarly, given an unmodified file, the first newline had to be moved to after the last line.

## Lessons

I actually found myself learning a lot during this small project:

- Data processing: Manually handling delimiters and weak semantic boundaries
- Building abstractions in order to solve a problem more easily/elegantly.
- Granular file specification using `std::fs::OpenOptions`
- Invoking subprocesses with `std::process::Command`
- Custom error creation and automatic conversion for ergonomic error handling
- Using temporary files and workspaces (disk space) to process data under RAM constraints.
- Files: correcting my mental model (files can't be inserted into at arbitrary locations).
    - Also, truncating a file doesn't necessarily reset its cursor position.
    
One lesson that doesn't quite fit with the others is that of state assumptions. It's important to define implicit assumptions about program state and ensure the program is forced to satisfy those assumptions when within a given function. For example, when reading from a file, I assumed I read from the beginning, but that wasn't the case due to a previous write operation. I had to ensure the given function manually set the cursor to gain proper functionality.


On to the next project.
