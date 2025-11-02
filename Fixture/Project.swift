import ProjectDescription

let project = Project(
    name: "Fixture",
    targets: [
        .target(
            name: "Fixture",
            destinations: .iOS,
            product: .app,
            bundleId: "dev.tuist.Fixture",
            infoPlist: .extendingDefault(
                with: [
                    "UILaunchScreen": [
                        "UIColorName": "",
                        "UIImageName": "",
                    ],
                ]
            ),
            buildableFolders: [
                "Fixture/Sources",
                "Fixture/Resources",
            ],
            dependencies: []
        ),
    ]
)
