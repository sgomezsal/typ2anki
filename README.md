# Typ2Anki

<div align="center">
  <a href="https://www.youtube.com/watch?v=simotHOIWNQ" target="_blank" style="display: inline-block; position: relative; text-decoration: none;">
    <img src="https://img.youtube.com/vi/simotHOIWNQ/maxresdefault.jpg" alt="Demo Video" style="border: 2px solid #ccc; border-radius: 15px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); width: 600px; height: auto;">
  </a>
  <p>
    <a href="https://www.youtube.com/watch?v=simotHOIWNQ" target="_blank" style="text-decoration: none; font-weight: bold; color: #0073e6;">
      ▶ Click to Watch the Demo Video on YouTube
    </a>
  </p>
</div>

**Typ2Anki** is a tool designed to integrate flashcard creation seamlessly into your Typst-based notes. By utilizing a custom Typst package, you can create cards directly in your notes and sync them effortlessly to a selected Anki deck. This enables you to transform study material into flashcards without disrupting your Typst workflow.

- Create flashcards directly within your Typst documents.
- Sync these flashcards to a chosen Anki deck effortlessly.
- Streamlined workflow for note-taking and spaced repetition learning.

---

## Table of Contents

1. **[Installation and Configuration](#installation-and-configuration)**

   - [Installing AnkiConnect](#installing-ankiconnect)
   - [Installing the Python Package](#installing-the-python-package)
   - [Installing the Typst Package](#installing-the-typst-package)

2. **[Usage](#usage)**

   - [Basic Workflow](#basic-workflow)
   - [Extra functionality and configuration](#extra-functionality-and-configuration)
   - [Customizing Cards](#customizing-cards)
   - [Example repositories](#example-repositories)

3. **[Troubleshooting](#troubleshooting)**

   - [Common Issues](#common-issues)

4. **[Roadmap](#roadmap)**

5. **[Future Plans](#future-plans)**

6. **[Contributing](#contributing)**

7. **[License](#license)**

---

## Installation and Configuration

### Installing AnkiConnect

1. Open Anki and navigate to **Tools > Add-ons**.
2. Click **Get Add-ons** and enter the following code to install AnkiConnect:

   ```
   2055492159
   ```

   Alternatively, visit the [AnkiConnect Add-on page](https://ankiweb.net/shared/info/2055492159) to learn more.
3. Restart Anki to activate the add-on.
4. Verify that AnkiConnect is running by visiting [http://localhost:8765](http://localhost:8765) in your browser. If it loads, the add-on is properly installed and functioning.

---

### Installing the Python Package

1. Make sure Python 3.8+ is installed on your system.
2. Install the Typ2Anki package using pip:

   ```bash
   pip install typ2anki
   ```

3. Verify the installation:

   ```bash
   typ2anki --help
   ```

#### Nix Flake

On systems with nix installed and flakes enabled, the
python package can be executed with the following command:

```sh
nix run github:sgomezsal/typ2anki
```

---

### Installing the Typst Package

#### Method 1: Using Package Manager (Recommended)

1. Add the Typ2Anki package to your Typst document:

   ```typst
   #import "@preview/typ2anki:0.1.0": *
   ```

#### Method 2: Manual Installation

If you encounter issues with the package import, you can set up the package manually:

1. Clone the repository:

   ```bash
   git clone https://github.com/sgomezsal/typ2anki
   cd typ2anki
   ```

2. Create the local package directory:

   ```bash
   mkdir -p ~/.local/share/typst/packages/local/typ2anki/0.1.0
   ```

3. Copy the package files (note the `-r` flag for recursive copy):

   ```bash
   cp -r src/ typst.toml ~/.local/share/typst/packages/local/typ2anki/0.1.0
   ```

4. Navigate to your flashcards directory:

   ```bash
   cd ~/Documents/Flashcards/  # or your preferred location
   ```

5. Create your `ankiconf.typ` file with basic configurations:

   ```typst
   // Custom imports for flashcards
   #import "@local/typ2anki:0.1.0": *
   #import "@preview/pkgs"

   #let conf(
     doc,
   ) = {
     // Custom configurations
     doc
   }
   ```

6. Create a new Typst document (e.g., `main.typ`):

   ```typst
   #import "ankiconf.typ": *
   #show: doc => conf(doc)

   #card(
     id: "001",
     target-deck: "Target-Deck",
     q: "Question",
     a: "Answer"
   )
   ```

7. Run typ2anki in your project directory:

   ```bash
   typ2anki .
   ```

---

## Usage

### Basic Workflow

1. Create a Typst file with flashcards:

   ```typst
   #card(
     id: "001",
     target-deck: "Typst",
     q: "What is Typst?",
     a: "A modern typesetting system."
   )
   ```

2. Create your `ankiconf.typ` file with basic configurations:

   ```typst
   // Custom imports for flashcards
   #import "@preview/typ2anki:0.1.0"
   #import "@preview/pkgs"

   #let conf(
     doc,
   ) = {
     // Custom configurations
     doc
   }
   ```

3. Use Typ2Anki to compile all files in the project directory:

   ```bash
   typ2anki ./path/to/your/project
   ```

4. Open your Anki deck to check the newly added flashcards.

---

### Extra functionality and configuration

- **Command line options**: Do `typ2anki --help` to see all available options.
  - Options include: specifying a max width for cards (to make sure they fit on phones - ex: `--max-card-width 430pt`), excluding files or decks
- **Configuration file**: You can create a `typ2anki.toml` file in your project directory to customize the behavior of `typ2anki`. This file can include default command line options for the project, so you don't have to specify them every time you run the command.
- **Compiling from .zip**: You can pass a `.zip` file to `typ2anki` to compile all Typst files inside it. This is useful so that if you use [typst.app](https://typst.app) you can download your project as a `.zip` and compile it with `typ2anki` without having to extract it first.

---

### Customizing Cards

To modify card appearance, you can define custom card logic:

```typst
#let custom-card(
  id: "",
  q: "",
  a: "",
  ..args
) = {
  card(
    id: id,
    q: q,
    a: a,
    container: true,
    show-labels: true
  )
}
```

---

### Example repositories:

- See [examples](https://github.com/sgomezsal/typ2anki/tree/main/examples) for a few cards
- Some math cards made using typ2anki: [itsvyle/typ2anki-demo](https://github.com/itsvyle/typ2anki-demo)

---

## Troubleshooting

### Common Issues

- **AnkiConnect not responding**:

  - Ensure Anki is running and AnkiConnect is installed correctly.

- **Typst file compilation errors**:
  - Check for syntax issues in your Typst file.
  - Ensure your `ankiconf.typ` contains the necessary imports and configurations.

---

## Roadmap

1. **Command to Delete Cards**: Implement a feature to remove specific cards from Anki.
2. **Efficiency Improvements**: Optimize the syncing process for speed and reliability.
3. **Support for Other Card Types**: Expand compatibility to include more complex card formats.

---

## Future Plans

- Enhance user experience with more robust error handling and syncing options.
- Broaden integration with Typst to support various output formats.

---

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests for bug fixes, feature enhancements, or documentation improvements.

---

## License

This project is licensed under the [MIT License](LICENSE).

---

Developed with ❤️ by [sgomezsal](https://github.com/sgomezsal). Let’s make learning efficient and enjoyable!
