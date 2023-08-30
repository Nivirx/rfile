# Stubby - Simple File and Text Sharing

Stubby is a simple yet powerful file and text sharing web service built using Rust and Rocket. Inspired by the functionality of Pastebin and Gist, Stubby aims to provide a similar experience with additional support for file uploads.

🚀 **Why Stubby?**

- Responsive UI
- File and text snippet uploads
- Built with Rust and Rocket for performance and safety

## Installation

### Prerequisites

- Rust programming language
- Cargo package manager

### Steps

1. Clone the repository:

    ```bash
    git clone https://github.com/Nivirx/Stubby.git
    ```

2. Navigate into the project directory:

    ```bash
    cd Stubby
    ```

3. Build and run the application using Cargo:

    ```bash
    cargo run
    ```

    Or build it first:

    ```bash
    cargo build --release
    ```

    And then run the resulting binary:

    ```bash
    ./target/release/stubby
    ```

## Usage

Open your web browser and navigate to `http://localhost:8000` (or the port you configured).

### Features

- Upload files
- Upload text snippets (coming soon)

## Dependencies

Stubby relies on the following dependencies as listed in the `Cargo.toml`:

- Rocket
- (other dependencies, if any)

Please refer to `Cargo.toml` for a complete list.

## Contribution

Feel free to submit issues, create pull requests or spread the word.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.

## Acknowledgments

- The Rust Language Team
- The Rocket Web Framework Team
