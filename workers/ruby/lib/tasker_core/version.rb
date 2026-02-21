# frozen_string_literal: true

module TaskerCore
  # Version synchronization with the core Rust crate
  # This should be kept in sync with the Cargo.toml version
  VERSION = '0.1.6'

  def self.version_info
    {
      version: VERSION
    }
  end
end
