# Tsunami with AWS SDK

An adapted version of [Tsunami](https://github.com/jonhoo/tsunami), modified to utilize the official AWS SDK instead of the Rusoto library. The original Tsunami's utilizes an outdated version of Rusoto that relied on a yanked library. Given the substantial API breaking changes in newer versions of Rusoto, transitioning to the official AWS SDK for EC2 presented a more reasonable solution.

## Overview

This tool automates the provisioning, configuration, and management of EC2 instances for running distributed jobs. It allows users to launch a set of pre-configured EC2 instances, execute setup commands via closures, and run jobs defined in a similar manner. After the jobs complete, the tool gracefully tears down the instances, ensuring no resources are left unused on AWS.
