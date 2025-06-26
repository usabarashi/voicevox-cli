# Pull Request

## Summary
<!-- Provide a brief summary of the changes -->

## Type of Change
<!-- Mark the relevant option with an "x" -->
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Code refactoring (no functional changes)
- [ ] Performance improvement
- [ ] CI/CD improvement
- [ ] Dependency update

## Related Issues
<!-- Link any related issues using "Fixes #123" or "Closes #123" -->
- Fixes #
- Related to #

## Changes Made
<!-- Describe the changes in detail -->

### Core Changes
- 

### Testing Changes
- 

### Documentation Changes
- 

## Testing
<!-- Describe how you tested your changes -->

### Manual Testing
<!-- Only test functionality changes beyond what `nix run .#ci` covers -->
- [ ] Tested feature-specific functionality (if applicable)
- [ ] Tested voice synthesis with actual audio output (if voice-related changes)
- [ ] Tested daemon-client communication (if daemon-related changes)

### Automated Testing
- [ ] Added new tests for new functionality

### Voice Model Testing
<!-- Only if changes affect voice model handling -->
- [ ] Tested with default voice (ずんだもん)
- [ ] Tested model installation/setup process (if model-related changes)

## Local CI Verification
<!-- Required: macOS CI is manual-only to reduce costs -->
- [ ] **`nix run .#ci` executed successfully** (includes build, tests, and verification)

## Build and Compatibility
<!-- Only check if `nix run .#ci` doesn't cover your changes -->
- [ ] Special build requirements tested (if applicable)
- [ ] Cross-platform compatibility verified (if applicable)

## Code Quality
<!-- Ensure code quality standards -->
- [ ] Code follows existing style guidelines
- [ ] Documentation updated if needed

## Performance Impact
<!-- Describe any performance implications -->
- [ ] No performance regression
- [ ] Performance improvement (describe below)
- [ ] Acceptable performance trade-off (describe below)

**Performance notes:**

## Breaking Changes
<!-- If this is a breaking change, describe the impact and migration path -->

### Impact
- 

### Migration Guide
- 

## Additional Notes
<!-- Any additional information, screenshots, or context -->

## Checklist
<!-- Final checklist before submitting -->
- [ ] I have read the contributing guidelines
- [ ] I have performed a self-review of my code
- [ ] I have commented my code where necessary
- [ ] **I have run `nix run .#ci` locally and all checks passed**
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] Any dependent changes have been merged and published

---

**Note**: Since macOS GitHub Actions CI is manual-only to reduce costs, please ensure you've run the complete local CI pipeline (`nix run .#ci`) before submitting your PR. This helps maintain code quality while minimizing CI expenses.