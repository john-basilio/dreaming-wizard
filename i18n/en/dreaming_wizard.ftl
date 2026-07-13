app-title = Dreaming Wizard
repository = https://github.com/john-basilio/dreaming-wizard
git-description = Git commit {$hash} on {$date}

## Header

# Header Start menus
hs_file = File
hs_help = Help
hs_canvas = Canvas

# Header Start: File
item_new = New Project
item_save = Save
item_load = Load Project

# Header Start: Help
item_about = About
about_comments = A narrative tool for creative storytelling.

# Header Start: Canvas
item_add_node = New Story Node

# Header Center: Title
project_title_prefix = Project:
# Header Center: Fallback title shown before the project has been saved
project-title-fallback = New Project
# Default project name/author saved when the user hasn't set one yet
project-name-fallback = Unknown
project-author-fallback = Unknown
# <--------------------->

## File dialogs

# Load dialog: window title (picks an existing project folder)
dialog-load-title = Choose Project Folder
# Save dialog: window title (the "Browse" button inside SaveProjectDialog;
# picks the *parent* folder a new project's own directory is created under)
dialog-save-title = Choose Location
# Avatar picker dialog: file filter label, restricting choices to images
dialog-image-filter-label = Images
# <--------------------->

## Popups

# SimplePopup: close ("X") button tooltip, shared by every use of the popup
popup-close-tooltip = Close
# Load Project popup: title, shared by both failure reasons below
popup-load-error-title = Couldn't Load Project
# Load Project popup: shown when the chosen folder has no project.json
popup-missing-project-message = This folder doesn't contain a project.json file.
# Load Project popup: shown when project.json exists but isn't valid JSON
popup-invalid-project-message = This folder's project.json file is invalid or corrupted.
# Save Project popup: title
popup-save-error-title = Couldn't Save Project
# Save Project popup: shown when the new project's directory couldn't be created
popup-save-dir-failed-message = Couldn't create the project folder. Check the location and try again.
# <--------------------->

## Save Project dialog (first save of a brand-new project)

save-dialog-title = Save New Project
save-dialog-name-label = Name
save-dialog-name-placeholder = My Project
save-dialog-path-label = Save as
save-dialog-path-placeholder = Choose a location...
save-dialog-browse = Browse
save-dialog-cancel = Cancel
save-dialog-confirm = Save Project
# Shown when Save Project is pressed before a location has been chosen
save-dialog-error-incomplete = Choose a location before saving.
# Shown when the resulting Name+location already exists and isn't empty
save-dialog-error-not-empty = Path is not empty.
# <--------------------->

# Toast shown after silently re-saving an already-open project
toast-saved = Saved ✓

## Nav tabs

# Canvas tab
nav-canvas-id = Canvas
# Character tab
nav-characters-id = Characters

# <--------------------->

## Canvas page

# Default title given to a newly added story node
node-default-title = New Node

# Editor: Label
editor-label = Editing
# Editor: Close
editor-close = Close
# Editor: Title label
editor-title-label = Title:
# Editor: Title input placeholder
editor-title-placeholder = Write node's title here... 

## Characters page

# Default name shown on a character card until it's been renamed
character-default-name = New Character

# Editor: Name label
editor-name-label = Name:
# Editor: Name input placeholder
editor-name-placeholder = Write character's name here...
# Editor: Comment label
editor-comment-label = Comment:
# Editor: Comment input placeholder
editor-comment-placeholder = Write a short comment here...
# Editor: Description label
editor-description-label = Description:
# Editor: Description input placeholder
editor-description-placeholder = Write character's description here...
