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

## Testing and Verification
<!-- Describe how you tested your changes -->

### Local CI Verification
<!-- Recommended: Run local CI for faster feedback -->
- [ ] **`nix run .#ci` executed successfully** (includes build, tests, and verification)

### Manual Testing
<!-- Only test functionality beyond automated CI -->
- [ ] Tested voice synthesis with actual audio output (if voice-related changes)
- [ ] Tested daemon-client communication (if daemon-related changes)
- [ ] Tested model installation/setup process (if model-related changes)

### New Tests Added
- [ ] Added new tests for new functionality (if applicable)

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

## Final Checklist
<!-- Complete before submitting -->
- [ ] I have performed a self-review of my code
- [ ] I have commented my code where necessary
- [ ] Any dependent changes have been merged and published

---

**Note**: Our comprehensive CI pipeline runs automatically on all PRs. Running `nix run .#ci` locally provides faster feedback and helps catch issues before pushing.