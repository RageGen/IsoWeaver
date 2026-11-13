# IsoWeaver user guide

## Open and browse an image

1. Launch IsoWeaver and choose **Open**.
2. Select an ISO image. The left pane shows folders; the right pane lists the active folder's entries.
3. Double-click a folder to open it. Use the search box to filter the active folder.
4. Open **Tools → Image Information** for the volume descriptor, El Torito report, and system-area report from xorriso.

## Edit an image

- **Add Files** and **Add Folder** add host content under the current image folder.
- **Delete Selected** removes selected entries after confirmation.
- **Edit → Rename Selected** renames a selected entry.
- **Edit → New Folder in Image** creates a directory in the active image folder.
- **Save** rebuilds the current image. **Save As** writes a new copy. The output is written to a temporary file and reopened before it replaces a destination.

Image changes remain staged until saved. Opening another image or quitting with pending changes asks before discarding them.

## Extract content

- Click a file to select it; hold **Ctrl** while clicking other files to add or remove them. **Extract Selected** extracts only the highlighted files. Selecting a folder extracts that folder and its contents.
- Choose **Extract All** to copy the whole image.
- IsoWeaver refuses to overwrite existing destination files. Choose an empty folder or remove/rename conflicting files first.

## Safety and limitations

IsoWeaver never runs programs from images. It rejects image paths that traverse outside the selected extraction directory. Boot inspection and replay use xorriso; the app verifies that previously detected boot structures remain present, but it cannot guarantee lossless preservation of every image's metadata. See [KNOWN_ISSUES.md](KNOWN_ISSUES.md) for details.
