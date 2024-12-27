# Typst2Anki

![Demo Video](assets/typst2anki.gif)

**Typst2Anki** is a tool designed to seamlessly integrate flashcard creation into your Typst-based notes. By utilizing a custom Typst package, you can create cards directly in your notes and sync them to a selected Anki deck with ease. This allows you to turn your study material into flashcards without leaving your Typst workflow.

## Features

- Create flashcards directly within your Typst documents.
- Sync these flashcards to a chosen Anki deck effortlessly.
- Streamlined workflow for note-taking and spaced repetition learning.

## Installation

1. Install Typst2Anki via pip:

```python
pip install typst2anki
```

2. Ensure you have the AnkiConnect plugin installed in Anki.

## Usage Example

To use Typst2Anki, simply navigate to your project directory containing Typst files with cards and run:

```bash
typst2anki ./path/to/your/project
```

This command will:

- Detect all `#cards` in the specified directory.
- Compile them using Typst.
- Send the compiled cards to your Anki deck.

Your Typst project must include an `ankiconf.typ` file containing all the libraries, rules, and configurations needed to compile your cards.

## Features and Roadmap

1. **Command to Delete Cards**: Implement a feature to remove specific cards from Anki.
2. **Efficiency Improvements**: Optimize the syncing process for speed and reliability.
3. **Support for Other Card Types**: Expand compatibility to include more complex card formats.
4. **Publish Typst Template**: Release a Typst package or template to simplify setup and usage.

## Future Plans

- Enhance user experience with more robust error handling and syncing options.
- Broaden integration with Typst to support various output formats.

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests for bug fixes, feature enhancements, or documentation improvements.

## License

This project is licensed under the [MIT License](LICENSE).

---

Developed with ❤️ by [sgomezsal](https://github.com/sgomezsal). Let’s make learning efficient and enjoyable!
