{
  # Development environment for sv2-apps that automatically patches Cargo.toml
  # to use a local stratum repository dependency instead of the GitHub version.
  #
  # Usage:
  #   1. Run `nix develop` to enter the development shell
  #   2. On first run, you'll be prompted for the local path to your stratum repository
  #   3. The path is stored in a symlink `stratum` for future sessions
  #   4. Cargo.toml files are automatically patched with [patch."https://github.com/stratum-mining/stratum"] section
  #
  # The flake provides commands for:
  #   Cargo.toml management:
  #     - patch-cargo-toml: Patch/update Cargo.toml to use local stratum repository
  #     - restore-cargo-toml: Remove the patch section from Cargo.toml
  #     - get-stratum-core-path: Get or set the cached stratum repository path
  #
  #   Bitcoin Core nodes:
  #     - bitcoin-mainnet: Launch Bitcoin Core on mainnet
  #     - bitcoin-testnet4: Launch Bitcoin Core on testnet4
  #     - bitcoin-signet: Launch Bitcoin Core on custom signet
  #
  #   Bitcoin CLI (with automatic authentication):
  #     - bitcoin-cli-mainnet: Bitcoin CLI for mainnet
  #     - bitcoin-cli-testnet4: Bitcoin CLI for testnet4
  #     - bitcoin-cli-signet: Bitcoin CLI for signet
  description = "Development environment for sv2-apps with automatic local stratum repository patching and Bitcoin Core commands";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        # Read Rust toolchain version from rust-toolchain.toml
        # This ensures the development environment uses the same Rust version as the project
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Visible symlink for IDE access (points to the stratum repository root)
        # This allows navigating the entire stratum repository from the IDE
        stratumRepoSymlink = "stratum";

        # Script: get-stratum-core-path
        # Prompts user for the local stratum repository path.
        # Creates/updates the stratum symlink pointing to the repository (for IDE navigation).
        # Outputs the absolute path to stratum-core (stratum/stratum-core) to stdout (for use in other scripts).
        # All prompts and status messages go to stderr to avoid interfering with piping.
        getStratumCorePath = pkgs.writeShellScriptBin "get-stratum-core-path" ''
          echo "Please provide the local path to the stratum repository:" >&2
          read -r STRATUM_REPO_PATH
          if [ -z "$STRATUM_REPO_PATH" ]; then
            echo "Error: Path cannot be empty" >&2
            exit 1
          fi
          
          # Expand ~ and resolve to absolute path
          STRATUM_REPO_PATH=$(realpath "$STRATUM_REPO_PATH" 2>/dev/null)
          if [ $? -ne 0 ] || [ ! -d "$STRATUM_REPO_PATH" ]; then
            echo "Error: Directory does not exist: $STRATUM_REPO_PATH" >&2
            exit 1
          fi
          
          # Verify stratum-core exists inside the repository
          STRATUM_CORE_PATH="$STRATUM_REPO_PATH/stratum-core"
          if [ ! -d "$STRATUM_CORE_PATH" ]; then
            echo "Error: stratum-core directory not found at: $STRATUM_CORE_PATH" >&2
            echo "Please provide the path to the stratum repository root (which contains stratum-core as a subdirectory)" >&2
            exit 1
          fi
          
          # Create or update visible symlink (stratum) pointing to repository
          # This allows accessing the entire stratum repository from the IDE
          if [ -L "${stratumRepoSymlink}" ] || [ -e "${stratumRepoSymlink}" ]; then
            CURRENT_REPO_TARGET=$(readlink -f "${stratumRepoSymlink}" 2>/dev/null)
            if [ "$CURRENT_REPO_TARGET" != "$STRATUM_REPO_PATH" ]; then
              rm -f "${stratumRepoSymlink}"
              ln -s "$STRATUM_REPO_PATH" "${stratumRepoSymlink}"
              echo "Updated symlink ${stratumRepoSymlink} -> $STRATUM_REPO_PATH" >&2
            fi
          else
            ln -s "$STRATUM_REPO_PATH" "${stratumRepoSymlink}"
            echo "Created symlink ${stratumRepoSymlink} -> $STRATUM_REPO_PATH" >&2
          fi
          
          echo "$STRATUM_CORE_PATH"
        '';

        # Script: patch-cargo-toml
        # Patches Cargo.toml files to use a local stratum-core dependency
        # instead of the GitHub version by adding/updating a [patch."https://github.com/stratum-mining/stratum"] section.
        #
        # This approach:
        #   - Doesn't modify the original dependency declaration
        #   - Uses Cargo's built-in patch mechanism (same as run-integration-tests.sh)
        #   - Works with any Cargo workspace structure
        #   - Calculates relative paths when possible for portability
        #
        # The script:
        #   1. Checks for the stratum symlink (prompts if missing)
        #   2. Validates the stratum repository path exists and contains stratum-core
        #   3. Resolves absolute path to stratum-core
        #   4. Adds or updates [patch."https://github.com/stratum-mining/stratum"] section in Cargo.toml
        patchCargoToml = pkgs.writeShellScriptBin "patch-cargo-toml" ''
          # Step 1: Get stratum-core path from stratum symlink or prompt user
          # Check if stratum symlink exists and derive stratum-core path from it
          if [ -L "${stratumRepoSymlink}" ]; then
            STRATUM_REPO_PATH=$(readlink -f "${stratumRepoSymlink}" 2>/dev/null)
            if [ -n "$STRATUM_REPO_PATH" ] && [ -d "$STRATUM_REPO_PATH" ]; then
              STRATUM_CORE_PATH="$STRATUM_REPO_PATH/stratum-core"
              if [ -d "$STRATUM_CORE_PATH" ]; then
                echo "Using stratum repository from ${stratumRepoSymlink} symlink -> stratum-core at: $STRATUM_CORE_PATH" >&2
              else
                echo "Warning: stratum-core not found at $STRATUM_CORE_PATH, will prompt for stratum repository path" >&2
                STRATUM_CORE_PATH=$(${getStratumCorePath}/bin/get-stratum-core-path)
              fi
            else
              echo "Warning: Symlink ${stratumRepoSymlink} is broken, will prompt for new stratum repository path" >&2
              STRATUM_CORE_PATH=$(${getStratumCorePath}/bin/get-stratum-core-path)
            fi
          else
            # No symlink exists, prompt user for stratum repository path
            STRATUM_CORE_PATH=$(${getStratumCorePath}/bin/get-stratum-core-path)
          fi
          
          # Step 2: Resolve absolute path for stratum-core
          STRATUM_CORE_ABS=$(realpath "$STRATUM_CORE_PATH" 2>/dev/null)
          if [ -z "$STRATUM_CORE_ABS" ] || [ ! -d "$STRATUM_CORE_ABS" ]; then
            echo "Error: Failed to resolve absolute path for stratum-core: $STRATUM_CORE_PATH" >&2
            exit 1
          fi
          
          # Step 3: Patch all workspace Cargo.toml files
          # Cargo patches need to be in workspace root Cargo.toml files to affect all members
          CARGO_TOMLS=(
            "./stratum-apps/Cargo.toml"
            "./pool-apps/Cargo.toml"
            "./miner-apps/Cargo.toml"
            "./integration-tests/Cargo.toml"
            "./bitcoin-core-sv2/Cargo.toml"
          )
          
          for CARGO_TOML in "''${CARGO_TOMLS[@]}"; do
            if [ ! -f "$CARGO_TOML" ]; then
              continue  # Skip if file doesn't exist
            fi
            
            # Add or update [patch."https://github.com/stratum-mining/stratum"] section with absolute path
            if grep -q '^\[patch\."https://github.com/stratum-mining/stratum"\]' "$CARGO_TOML"; then
              # Update existing patch section - remove old stratum-core line and add new one
              ${pkgs.gnused}/bin/sed -i '/^stratum-core = {path =/d' "$CARGO_TOML"
              # Add the new patch line right after the patch section header
              ${pkgs.gnused}/bin/sed -i '/^\[patch\."https:\/\/github.com\/stratum-mining\/stratum"\]/a\
stratum-core = {path = "'"$STRATUM_CORE_ABS"'"}
' "$CARGO_TOML"
              echo "Updated [patch.\"https://github.com/stratum-mining/stratum\"] section in $CARGO_TOML" >&2
            else
              # Append patch section at the end
              cat >> "$CARGO_TOML" << EOF

# Override dependencies with local paths
[patch."https://github.com/stratum-mining/stratum"]
stratum-core = {path = "$STRATUM_CORE_ABS"}
EOF
              echo "Added [patch.\"https://github.com/stratum-mining/stratum\"] section to $CARGO_TOML" >&2
            fi
          done
          
          echo "Patched all Cargo.toml files to use local stratum-core at: $STRATUM_CORE_ABS" >&2
        '';

        # Script: restore-cargo-toml
        # Removes the [patch."https://github.com/stratum-mining/stratum"] section from all Cargo.toml files
        # to restore the original state (using GitHub version of stratum-core).
        #
        # The script:
        #   1. Removes the stratum-core patch line from all Cargo.toml files
        #   2. Checks if [patch."https://github.com/stratum-mining/stratum"] section is now empty
        #   3. Removes the empty section header if no other patches remain
        #   4. Handles edge case where the patch section is the last section
        restoreCargoToml = pkgs.writeShellScriptBin "restore-cargo-toml" ''
          # Restore all Cargo.toml files
          CARGO_TOMLS=(
            "./stratum-apps/Cargo.toml"
            "./pool-apps/Cargo.toml"
            "./miner-apps/Cargo.toml"
            "./integration-tests/Cargo.toml"
            "./bitcoin-core-sv2/Cargo.toml"
          )
          
          for CARGO_TOML in "''${CARGO_TOMLS[@]}"; do
            if [ ! -f "$CARGO_TOML" ]; then
              continue  # Skip if file doesn't exist
            fi
          
            # Remove stratum-core patch line if it exists
            if grep -q '^stratum-core = {path =' "$CARGO_TOML"; then
              ${pkgs.gnused}/bin/sed -i '/^stratum-core = {path =/d' "$CARGO_TOML"
              echo "Removed stratum-core patch from $CARGO_TOML" >&2
              
              # Clean up empty [patch."https://github.com/stratum-mining/stratum"] section if no other patches remain
            # We need to check if the section is truly empty (only header + blank lines/comments)
            # This handles the case where the patch section is the last section in the file
            if grep -q '^\[patch\."https://github.com/stratum-mining/stratum"\]' "$CARGO_TOML"; then
              # Get line number of patch section
              PATCH_LINE=$(grep -n '^\[patch\."https://github.com/stratum-mining/stratum"\]' "$CARGO_TOML" | head -1 | cut -d: -f1)
              # Extract content after patch header, stopping at next [section] or EOF
              # Filter out blank lines and comments to check if section is truly empty
              SECTION_CONTENT=$(tail -n +$((PATCH_LINE + 1)) "$CARGO_TOML" | ${pkgs.gnused}/bin/sed '/^\[/q' | grep -v '^[[:space:]]*$' | grep -v '^[[:space:]]*#')
              if [ -z "$SECTION_CONTENT" ]; then
                # Check if the line before the patch section is our comment before removing
                # We need to check before removal because line numbers will shift
                REMOVE_COMMENT=false
                if [ $PATCH_LINE -gt 1 ]; then
                  PREV_LINE=$(sed -n "$((PATCH_LINE - 1))p" "$CARGO_TOML")
                  if echo "$PREV_LINE" | grep -q '^# Override dependencies with local paths'; then
                    REMOVE_COMMENT=true
                  fi
                fi
                # Remove the patch section header (need to escape the URL in sed)
                ${pkgs.gnused}/bin/sed -i '/^\[patch\."https:\/\/github.com\/stratum-mining\/stratum"\]/d' "$CARGO_TOML"
                # Remove the comment line if it was our patch comment
                if [ "$REMOVE_COMMENT" = true ]; then
                  ${pkgs.gnused}/bin/sed -i '/^# Override dependencies with local paths$/d' "$CARGO_TOML"
                fi
                # Clean up trailing blank lines at end of file
                ${pkgs.gnused}/bin/sed -i -e :a -e '/^\n*$/{$d;N;ba' -e '}' "$CARGO_TOML"
                echo "Removed empty [patch.\"https://github.com/stratum-mining/stratum\"] section from $CARGO_TOML" >&2
              fi
            fi
            else
              echo "No stratum-core patch found in $CARGO_TOML" >&2
            fi
          done
        '';

        # Bitcoin RPC credentials
        rpcUser = "username";
        rpcPassword = "password";

        # Common Bitcoin CLI arguments for all networks
        commonBitcoinArgs = [
          "-server=1"
          "-rpcuser=${rpcUser}"
          "-rpcpassword=${rpcPassword}"
          "-rpcbind=0.0.0.0"
          "-rpcallowip=0.0.0.0/0"
          "-prune=555"
          "-ipcbind=unix"
        ];

        # Script: bitcoin-mainnet
        # Launches Bitcoin Core in mainnet mode
        bitcoinMainnet = pkgs.writeShellScriptBin "bitcoin-mainnet" ''
          exec ${pkgs.bitcoin}/bin/bitcoin -m node ${pkgs.lib.concatStringsSep " " commonBitcoinArgs}
        '';

        # Script: bitcoin-testnet4
        # Launches Bitcoin Core in testnet4 mode
        bitcoinTestnet4 = pkgs.writeShellScriptBin "bitcoin-testnet4" ''
          exec ${pkgs.bitcoin}/bin/bitcoin -m node -testnet4 ${pkgs.lib.concatStringsSep " " commonBitcoinArgs}
        '';

        # Script: bitcoin-signet
        # Launches Bitcoin Core in signet mode with custom network configuration
        bitcoinSignet = pkgs.writeShellScriptBin "bitcoin-signet" ''
          exec ${pkgs.bitcoin}/bin/bitcoin -m node -signet \
            -signetchallenge=51 \
            -connect=185.130.45.51 \
            ${pkgs.lib.concatStringsSep " " commonBitcoinArgs}
        '';

        # Script: bitcoin-cli-mainnet
        # Bitcoin CLI wrapper for mainnet with automatic RPC authentication
        bitcoinCliMainnet = pkgs.writeShellScriptBin "bitcoin-cli-mainnet" ''
          exec ${pkgs.bitcoin}/bin/bitcoin-cli -rpcuser=${rpcUser} -rpcpassword=${rpcPassword} "$@"
        '';

        # Script: bitcoin-cli-testnet4
        # Bitcoin CLI wrapper for testnet4 with automatic RPC authentication
        bitcoinCliTestnet4 = pkgs.writeShellScriptBin "bitcoin-cli-testnet4" ''
          exec ${pkgs.bitcoin}/bin/bitcoin-cli -testnet4 -rpcuser=${rpcUser} -rpcpassword=${rpcPassword} "$@"
        '';

        # Script: bitcoin-cli-signet
        # Bitcoin CLI wrapper for signet with automatic RPC authentication
        bitcoinCliSignet = pkgs.writeShellScriptBin "bitcoin-cli-signet" ''
          exec ${pkgs.bitcoin}/bin/bitcoin-cli -signet -rpcuser=${rpcUser} -rpcpassword=${rpcPassword} "$@"
        '';

      in
      {
        # Development shell that provides:
        #   - Rust toolchain (from rust-toolchain.toml)
        #   - Cargo, rustfmt, clippy
        #   - Git for version control
        #   - Custom scripts for managing stratum-core patching
        #   - Automatic Cargo.toml patching on shell entry
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain  # Rust compiler and standard tools
            cargo          # Rust package manager
            rustfmt        # Code formatter
            clippy         # Linter
            git            # Version control
            getStratumCorePath  # Script to get/set stratum repository path
            patchCargoToml      # Script to patch Cargo.toml
            restoreCargoToml    # Script to restore original Cargo.toml
            gnused         # Required for sed operations in scripts
            bitcoin        # Bitcoin Core
            bitcoinMainnet # Script to launch Bitcoin Core in mainnet mode
            bitcoinTestnet4 # Script to launch Bitcoin Core in testnet4 mode
            bitcoinSignet  # Script to launch Bitcoin Core in custom signet mode
            bitcoinCliMainnet # Bitcoin CLI wrapper for mainnet
            bitcoinCliTestnet4 # Bitcoin CLI wrapper for testnet4
            bitcoinCliSignet # Bitcoin CLI wrapper for signet
          ];

          # Shell hook runs automatically when entering `nix develop`
          # Checks if Cargo.toml files are already patched, and patches them if not
          # Also ensures the stratum symlink exists for IDE navigation
          shellHook = ''
            echo "=== SV2 Apps Development Environment ==="
            echo ""
            
            # Check if any workspace Cargo.toml is already patched
            if grep -q '^\[patch\."https://github.com/stratum-mining/stratum"\]' ./stratum-apps/Cargo.toml 2>/dev/null || \
               grep -q '^\[patch\."https://github.com/stratum-mining/stratum"\]' ./pool-apps/Cargo.toml 2>/dev/null || \
               grep -q '^\[patch\."https://github.com/stratum-mining/stratum"\]' ./miner-apps/Cargo.toml 2>/dev/null || \
               grep -q '^\[patch\."https://github.com/stratum-mining/stratum"\]' ./bitcoin-core-sv2/Cargo.toml 2>/dev/null; then
              echo "✓ Cargo.toml files already have [patch.\"https://github.com/stratum-mining/stratum\"] section."
              
              # Extract stratum-core path from any Cargo.toml and create symlink if needed
              PATCH_PATH=$(grep '^stratum-core = {path =' ./stratum-apps/Cargo.toml ./pool-apps/Cargo.toml ./miner-apps/Cargo.toml ./bitcoin-core-sv2/Cargo.toml 2>/dev/null | head -1 | sed -n "s/.*path = \"\([^\"]*\)\".*/\1/p")
              if [ -n "$PATCH_PATH" ]; then
                # Resolve to absolute path (handles relative paths)
                CARGO_TOML_DIR=$(dirname "$(realpath ./stratum-apps/Cargo.toml 2>/dev/null)")
                if [ -n "$CARGO_TOML_DIR" ]; then
                  # Handle relative paths - resolve relative to Cargo.toml directory
                  if [[ "$PATCH_PATH" != /* ]]; then
                    STRATUM_CORE_ABS=$(realpath "$CARGO_TOML_DIR/$PATCH_PATH" 2>/dev/null)
                  else
                    STRATUM_CORE_ABS=$(realpath "$PATCH_PATH" 2>/dev/null)
                  fi
                  
                  if [ -n "$STRATUM_CORE_ABS" ]; then
                    # Determine the correct stratum repo root
                    # If the path points to a directory that contains stratum-core as a subdirectory,
                    # use that directory. Otherwise, assume it points to stratum-core and use its parent.
                    if [ -d "$STRATUM_CORE_ABS" ]; then
                      if [ -d "$STRATUM_CORE_ABS/stratum-core" ]; then
                        # Path points to stratum repo root, use it directly
                        STRATUM_REPO_PATH="$STRATUM_CORE_ABS"
                      else
                        # Path points to stratum-core directory, use its parent
                        STRATUM_REPO_PATH=$(dirname "$STRATUM_CORE_ABS")
                      fi
                      # Create or update stratum symlink
                      if [ ! -L "${stratumRepoSymlink}" ] || [ "$(readlink -f "${stratumRepoSymlink}" 2>/dev/null)" != "$STRATUM_REPO_PATH" ]; then
                        rm -f "${stratumRepoSymlink}"
                        ln -s "$STRATUM_REPO_PATH" "${stratumRepoSymlink}"
                        echo "Created/updated ${stratumRepoSymlink} symlink -> $STRATUM_REPO_PATH" >&2
                      fi
                    else
                      echo "Warning: Path from Cargo.toml does not exist: $STRATUM_CORE_ABS" >&2
                    fi
                  fi
                fi
              fi
            else
              echo "Patching Cargo.toml to use local stratum repository..."
              if patch-cargo-toml; then
                echo "✓ Successfully patched Cargo.toml"
              else
                echo "✗ Failed to patch Cargo.toml"
              fi
            fi
            
            echo ""
            echo "Available commands:"
            echo "  Cargo.toml management:"
            echo "    patch-cargo-toml      - Patch/update Cargo.toml to use local stratum repository"
            echo "    restore-cargo-toml    - Remove patch section, restore GitHub dependency"
            echo "    get-stratum-core-path - Get or set the stratum repository path (creates/updates symlink)"
            echo ""
            echo "  Bitcoin Core nodes:"
            echo "    bitcoin-mainnet       - Launch Bitcoin Core on mainnet"
            echo "    bitcoin-testnet4      - Launch Bitcoin Core on testnet4"
            echo "    bitcoin-signet        - Launch Bitcoin Core on custom signet"
            echo ""
            echo "  Bitcoin CLI (with automatic authentication):"
            echo "    bitcoin-cli-mainnet   - Bitcoin CLI for mainnet"
            echo "    bitcoin-cli-testnet4  - Bitcoin CLI for testnet4"
            echo "    bitcoin-cli-signet    - Bitcoin CLI for signet"
            echo ""
            echo "The stratum repository is accessible via symlink: ${stratumRepoSymlink}"
            echo "To change the stratum repository path, run: patch-cargo-toml"
            echo "To remove the patch, run: restore-cargo-toml"
            echo ""
          '';
        };
      }
    );
}
