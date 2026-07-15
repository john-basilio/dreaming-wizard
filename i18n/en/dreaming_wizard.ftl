app-title = Dreaming Wizard
repository = https://github.com/john-basilio/dreaming-wizard
git-description = Git commit {$hash} on {$date}

## Header

# Header Start menus
hs_file = File
hs_help = Help
hs_action = Action

# Header Start: File
item_new = New Project
item_save = Save
item_load = Load Project

# Header Start: Help
item_about = About
# Jumps to (and highlights) the Preferences page's Language setting
item_language = Language
about_comments = A narrative tool for creative storytelling.

# Header Start: Action
item_add_node = Add Node
item_add_character = Add Character
item_find = Find

# Header Center: Title
project_title_prefix = Project:
# Header Center: Title prefix shown instead of the above while there's an
# unsaved change (see AppModel::is_project_dirty)
project-title-unsaved-prefix = *Unsaved:
# Header Center: Fallback title shown before the project has been saved
project-title-fallback = New Project
# Default project name/author saved when the user hasn't set one yet
project-name-fallback = Unknown
project-author-fallback = Unknown
# <--------------------->

## File dialogs

# Load dialog: window title (picks an existing project folder)
dialog-load-title = Choose Project Folder
# Browse dialog: window title (the "Browse" button inside NewProjectDialog;
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
# Load Project popup: shown when the chosen folder has neither a
# project.toml manifest nor a legacy project.json
popup-missing-project-message = This folder doesn't contain a project.toml (or legacy project.json) file.
# Load Project popup: shown when the project files exist but couldn't be parsed
popup-invalid-project-message = This folder's project files are invalid or corrupted.
# Save Project popup: title
popup-save-error-title = Couldn't Save Project
# Save Project popup: shown when the new project's directory couldn't be created
popup-save-dir-failed-message = Couldn't create the project folder. Check the location and try again.
# <--------------------->

## New Project dialog (startup with no project to reopen, and File → New)

new-project-title = New Project
new-project-name-label = Name
new-project-name-placeholder = My Project
new-project-path-label = Location
new-project-path-placeholder = Choose a location...
new-project-author-label = Author
new-project-author-placeholder = Who's writing this story?
new-project-comment-label = Comment
new-project-comment-placeholder = A short description of the project...
new-project-browse = Browse
new-project-cancel = Cancel
new-project-create = Create Project
# Runs the same folder picker as File → Load; a successful load replaces
# this dialog entirely
new-project-open-existing = Open existing…
# Shown when Create Project is pressed before a name+location are set
new-project-error-incomplete = Choose a name and location first.
# Shown when the resulting name+location already exists and isn't empty
new-project-error-not-empty = That folder already exists and isn't empty.
# <--------------------->

# Toast shown after silently re-saving an already-open project
toast-saved = Saved ✓

## Nav tabs

# Canvas tab
nav-canvas-id = Canvas
# Character tab
nav-characters-id = Characters
# Preferences tab
nav-settings-id = Preferences

# <--------------------->

## Preferences page

# Section 1: the open project's own metadata (stored in project.json)
prefs-section-project = Project
prefs-author = Author name
prefs-author-placeholder = Who's writing this story?
prefs-comment = Comment
prefs-comment-placeholder = A short description of the project...
prefs-repository = Repository link
prefs-repository-placeholder = https://github.com/you/your-story

# Section 2: development workflow
prefs-section-development = Development
prefs-autosave = Autosave
prefs-autosave-interval = Autosave interval (minutes)
prefs-reopen = Reopen last project on launch

# Section 3: editor behavior
prefs-section-editor = Editor
prefs-zoom-sensitivity = Zoom sensitivity
prefs-preview-lines = Collapsed preview lines
prefs-confirm-nodes = Confirm before deleting nodes
prefs-confirm-characters = Confirm before deleting characters
prefs-confirm-blocks = Confirm before deleting blocks
prefs-language = Language
# The language radio that follows the system locale instead of overriding
prefs-language-system = System default

## Canvas page

# Default title given to a newly added story node
node-default-title = New Node

# Editor: Label
editor-label = Editing
# Editor: Delete (shared by the story node and character card editors)
editor-delete = Delete
# Editor: Close
editor-close = Close
# Editor: enters title-editing mode (story node editor header)
editor-edit = Edit
# Editor: commits the title being edited
editor-save = Save
# Editor: discards the title being edited
editor-cancel = Cancel
# Editor: Title label
editor-title-label = Title:
# Editor: Title input placeholder
editor-title-placeholder = Write node's title here...

# Floating "+" button tooltip on the canvas page
tooltip-add-node = Add Node

# Hover button on a node: makes it the story's entry point (start node)
tooltip-set-start-node = Set as start node

## Story node editor: content blocks

# Block kind labels — each block card's header, and the add-block buttons
block-narration = Narration
block-dialogue = Dialogue
block-choice = Choice
block-directive = Directive
block-note = Note

# The narrator persona label shown under narration blocks' fixed avatar
block-narrator-label = Narrator

# Prose body placeholders
block-narration-placeholder = Write narration here...
block-dialogue-placeholder = Write the spoken line here...
block-note-placeholder = Write an author-only note here (never shown to players)...

# Shared "nothing selected" entry of the inline speaker/target dropdowns
dropdown-none = (None)

# Directive: command/argument input placeholders
block-directive-command-placeholder = command
block-directive-argument-placeholder = argument

# Choice: option label placeholder and the add-option button
choice-option-placeholder = Choice text...
choice-add-option = Add option

# Block bubble button tooltips
tooltip-remove-block = Remove block
tooltip-remove-option = Remove option

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

# Floating "+" button tooltip on the characters page
tooltip-add-character = Add Character

## Delete confirmation (shared ConfirmDialog, both pages)

# Hover-delete button tooltip, shared by both story nodes and character cards
tooltip-delete = Delete
confirm-dialog-cancel = Cancel
confirm-dialog-delete = Delete
# Checkbox offering to disable this kind of confirmation (see the
# per-entity toggles on the Preferences page)
confirm-dont-ask-again = Don't ask again
confirm-delete-node-title = Delete Story Node?
confirm-delete-node-message = Are you sure you want to delete "{$title}"? This can't be undone.
confirm-delete-character-title = Delete Character?
confirm-delete-character-message = Are you sure you want to delete "{$name}"? This can't be undone.
confirm-delete-block-title = Delete Block?
confirm-delete-block-message = Are you sure you want to delete this block? This can't be undone.

## Unsaved changes warning (shared UnsavedChangesDialog: editor close, app exit)

unsaved-changes-title = Unsaved Changes
unsaved-changes-message = You have unsaved changes. Save before closing?
unsaved-changes-save = Save
unsaved-changes-discard = Discard
unsaved-changes-cancel = Cancel

## Find panel (Ctrl+F)

find-label = Find:
find-placeholder = Search...
find-target-node = Node
find-target-character = Character
find-close-tooltip = Close
