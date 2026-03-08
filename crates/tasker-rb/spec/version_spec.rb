# frozen_string_literal: true

require 'spec_helper'

RSpec.describe 'TaskerCore version constants' do
  describe 'VERSION' do
    it 'is a string matching semver format' do
      expect(TaskerCore::VERSION).to be_a(String)
      expect(TaskerCore::VERSION).to match(/\A\d+\.\d+\.\d+\z/)
    end
  end

  describe '.version_info' do
    subject(:info) { TaskerCore.version_info }

    it 'returns a hash with version key' do
      expect(info).to be_a(Hash)
      expect(info).to have_key(:version)
    end

    it 'contains version string' do
      expect(info[:version]).to eq(TaskerCore::VERSION)
    end
  end
end
