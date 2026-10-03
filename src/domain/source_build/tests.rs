use super::*;

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

/// Every expectation is what `nvm_get_make_jobs` of the real `nvm.sh` prints.
#[test]
fn a_natural_number_of_jobs_is_used_as_it_is() {
    let said = jobs(Some("4"), Some(18));
    assert_eq!(
        (said.jobs, said.stdout, said.stderr),
        (4, vec!["number of `make` jobs: 4".to_owned()], vec![])
    );
}

#[test]
fn without_a_request_one_core_is_left_free_when_there_are_more_than_two() {
    let said = jobs(None, Some(18));
    assert_eq!(said.jobs, 17);
    assert_eq!(
        said.stdout,
        [
            "Detected that you have 18 CPU core(s)",
            "Running with 17 threads to speed up the build"
        ]
    );
    assert_eq!(jobs(None, Some(3)).jobs, 2);
}

#[test]
fn two_cores_or_fewer_build_with_one_job() {
    for cores in [1, 2] {
        let said = jobs(None, Some(cores));
        assert_eq!(said.jobs, 1);
        assert_eq!(
            said.stdout,
            [
                format!("Detected that you have {cores} CPU core(s)"),
                "Number of CPU core(s) less than or equal to 2, running in single-threaded mode"
                    .to_owned()
            ]
        );
    }
}

#[test]
fn a_request_that_is_not_a_natural_number_is_reported_and_the_cores_decide() {
    for bad in ["abc", "0", "-3", "1.5"] {
        let said = jobs(Some(bad), Some(8));
        assert_eq!(said.jobs, 7, "{bad}");
        assert_eq!(
            said.stderr,
            [format!(
                "{bad} is invalid for number of `make` jobs, must be a natural number"
            )]
        );
    }
    assert!(jobs(Some(""), Some(8)).stderr.is_empty());
}

#[test]
fn unknown_cores_mean_one_job_and_a_plea_to_report_it() {
    for cores in [None, Some(0)] {
        let said = jobs(None, cores);
        assert_eq!(said.jobs, 1);
        assert!(said.stdout.is_empty());
        assert_eq!(
            said.stderr,
            [
                "Can not determine how many core(s) are available, running in single-threaded mode.",
                "Please report an issue on GitHub to help us make nvm run faster on your computer!",
            ]
        );
    }
}

#[test]
fn clang_reports_its_version_in_one_of_two_places() {
    assert_eq!(
        clang_version("clang version 15.0.7 (Fedora 15.0.7-1.fc37)\nTarget: x86_64"),
        Some((15, 0))
    );
    assert_eq!(
        clang_version("Apple clang version 15.0.0 (clang-1500.1.0.2.5)\nTarget: arm64"),
        Some((15, 0))
    );
    assert_eq!(
        clang_version("clang version 3.5.2-1ubuntu1 (tags/RELEASE_352/final)"),
        Some((3, 5))
    );
    assert_eq!(clang_version("gcc (GCC) 13.2.0"), None);
    assert_eq!(clang_version(""), None);
}

#[test]
fn macos_and_the_bsds_name_the_compilers_and_linux_does_not() {
    let names = vec!["CC=cc".to_owned(), "CXX=c++".to_owned()];
    for os in [Os::Darwin, Os::FreeBsd, Os::OpenBsd] {
        assert_eq!(compiler(os, None, None, None).words, names);
    }
    assert!(compiler(Os::Linux, None, None, None).words.is_empty());
    assert!(compiler(Os::Aix, None, None, None).words.is_empty());
}

#[test]
fn the_compilers_in_the_environment_are_used_and_empty_ones_are_not() {
    let words = compiler(Os::Darwin, None, Some("gcc-13"), Some("")).words;
    assert_eq!(words, ["CC=gcc-13", "CXX=c++"]);
}

#[test]
fn clang_3_5_or_later_is_used_when_cc_or_cxx_is_missing() {
    let found = compiler(Os::Linux, Some((15, 0)), None, Some("g++"));
    assert!(found.clang_note);
    assert_eq!(found.words, ["CC=cc", "CXX=g++"]);
    assert!(!compiler(Os::Linux, Some((15, 0)), Some("gcc"), Some("g++")).clang_note);
    assert!(!compiler(Os::Linux, Some((3, 4)), None, None).clang_note);
    assert!(!compiler(Os::Linux, None, None, None).clang_note);
}

#[test]
fn the_bsds_and_aix_use_gmake_and_old_node_gets_a_shell() {
    assert_eq!(make(Os::Linux, &version("v20.10.0")), ("make", vec![]));
    assert_eq!(make(Os::FreeBsd, &version("v20.10.0")).0, "gmake");
    assert_eq!(make(Os::Aix, &version("v20.10.0")).0, "gmake");
    assert_eq!(
        make(Os::Darwin, &version("v0.10.48")),
        ("make", vec!["SHELL=/bin/sh".to_owned()])
    );
    assert!(make(Os::Linux, &version("v0.12.0")).1.is_empty());
}
