Please use FileSystem tools to finish the following task:

Find every line that contains `TODO` (in capitals) in any file under `src/`, at any depth. Write `todo.txt` in the main directory with one line per match: the file's path relative to the main directory, a colon, the line number (counting from 1), a colon, a space, and the line with leading and trailing whitespace removed. For example `src/app.py:3: # TODO: read the port from config`. Sort by path, then line number. End the file with a newline.
