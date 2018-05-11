pipeline {
  agent any

  triggers {
    pollSCM('H/5 * * * *')
  }

  stages {
    stage('Build') {
      steps {
        sh'''
          #!/bin/bash
          cd padre
          npm install
          cd ..
          docker build -t vim-padre/test-container .
        '''
      }
    }

    stage('Python 2 Unit Testing') {
      steps {
        sh'''
          #!/bin/bash
          cd pythonx
          python -m unittest discover -v test/
        '''
      }
    }

    stage('Python 3 Unit Testing') {
      steps {
        sh'''
          #!/bin/bash
          cd pythonx
          python3 -m unittest discover -v test/
        '''
      }
    }

    stage('PADRE Unit Tests') {
      steps {
        sh'''
          #!/bin/bash
          cd padre
          npm test
        '''
      }
    }

    stage('Integration Tests') {
      steps {
        sh'''
          #!/bin/bash
          set +x
          . /var/lib/jenkins/robot/bin/activate
          cd padre/integration/
          robot *.robot
        '''
      }
    }

    stage('Vader Test') {
      steps {
        sh'''
          #!/bin/bash
          docker run -a stderr -e VADER_OUTPUT_FILE=/dev/stderr --rm vim-padre/test-container /vim-build/bin/vim-v8.0.0027 -u ./test/vimrc "+Vader! test/*.vader" 2>&1
        '''
      }
    }
  }

  post {
    always {
      step([
        $class: 'hudson.plugins.robot.RobotPublisher',
        outputPath: './',
        passThreshold : 100,
        unstableThreshold: 100,
        otherFiles: '',
        reportFileName: 'padre/integration/report*.html',
        logFileName: 'padre/integration/log*.html',
        outputFileName: 'padre/integration/output*.xml'
      ])
    }
  }
}
