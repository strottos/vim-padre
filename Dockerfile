FROM tweekmonster/vim-testbed:latest

RUN apk --update add bash gcc git go libc-dev python py-pip && rm -rf /var/cache/apk/* /tmp/* /var/tmp/* && pip install six

RUN install_vim -tag v8.0.0027 -py -build

RUN git clone https://github.com/junegunn/vader.vim /vimfiles
ENV GOPATH=/usr/local
RUN go get -v github.com/strottos/padre
USER vimtest
WORKDIR /home/vimtest
COPY test/ /home/vimtest/test/
COPY . /testplugin
